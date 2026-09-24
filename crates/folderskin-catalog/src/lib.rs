//! FolderSkin's community catalog: every published pack and skin in one SQLite database with
//! full-text search, and the published tree around it (`head.json`, content-addressed pictures,
//! thumbnails and pack manifests).
//!
//! `folderskin-tools packs catalog` builds the tree from `community/packs`; the app downloads the
//! catalog once per generation and searches it on the user's own computer, so a search over tens
//! of thousands of packs costs a few milliseconds and no request at all.

pub mod build;
pub mod search;
pub mod tree;

pub use build::PackRecord;
pub use search::{Catalog, Facet, PackRow, Query, Results, SkinHit, Sort};

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(
        id: &str,
        name: &str,
        author: &str,
        tags: &[&str],
        added: i64,
        skins: &[&str],
    ) -> PackRecord {
        PackRecord {
            id: id.into(),
            name: name.into(),
            author: author.into(),
            license: "CC0-1.0".into(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            hash: format!("{added:016x}"),
            manifest: tree::sha256_hex(id.as_bytes()),
            added,
            count: skins.len(),
            bytes: 1000 * skins.len() as u64,
            skin_names: skins.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn sample() -> Vec<PackRecord> {
        vec![
            pack(
                "classic-art",
                "Classic Art",
                "prajwal-svm",
                &["classic art", "painting"],
                300,
                &[
                    "Mona Lisa",
                    "The Starry Night",
                    "View of Toledo",
                    "Composition VIII",
                ],
            ),
            pack(
                "colours",
                "Colours",
                "prajwal-svm",
                &["colour"],
                100,
                &["Blue", "Orange", "Purple", "Green"],
            ),
            pack(
                "greek-art",
                "Greek Art",
                "someone",
                &["classic art", "sculpture"],
                200,
                &["Parthenon", "Poseidon", "Marble youth"],
            ),
            pack(
                "night-prints",
                "Night prints",
                "hokusai-fan",
                &["woodblock", "night"],
                400,
                &["Starling", "Night heron", "Moon over Edo"],
            ),
            pack(
                "cafe-noir",
                "Café noir",
                "barista",
                &["coffee"],
                50,
                &["Espresso"],
            ),
        ]
    }

    fn catalog(packs: &[PackRecord]) -> Catalog {
        Catalog::from_connection(build::in_memory(packs).unwrap()).unwrap()
    }

    fn ids(r: &Results) -> Vec<&str> {
        r.packs.iter().map(|p| p.id.as_str()).collect()
    }

    /// The ids in `r`, in id order, for a match whose ranking the test doesn't care about.
    fn id_set(r: &Results) -> Vec<&str> {
        let mut ids = ids(r);
        ids.sort();
        ids
    }

    fn find(c: &Catalog, q: &str) -> Results {
        c.search(&Query {
            q,
            limit: 50,
            ..Query::default()
        })
        .unwrap()
    }

    #[test]
    fn the_same_packs_make_the_same_bytes_in_any_order() {
        let a = build::to_bytes(&sample()).unwrap();
        let mut shuffled = sample();
        shuffled.reverse();
        let b = build::to_bytes(&shuffled).unwrap();
        assert_eq!(a, b, "the catalog is deterministic");
        assert!(a.starts_with(b"SQLite format 3\0"));
        let mut changed = sample();
        changed[1].skin_names[0] = "Navy".into();
        assert_ne!(a, build::to_bytes(&changed).unwrap());
        let mut twice = sample();
        twice.push(sample()[0].clone());
        assert!(build::to_bytes(&twice).unwrap_err().contains("same id"));
    }

    #[test]
    fn a_catalog_file_opens_read_only_and_says_what_it_holds() {
        let dir = std::env::temp_dir().join(format!("fs-catalog-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("c.sqlite");
        std::fs::write(&path, build::to_bytes(&sample()).unwrap()).unwrap();
        let c = Catalog::open(&path).unwrap();
        assert_eq!(c.counts(), (5, 15));
        assert_eq!(ids(&find(&c, "mona")), ["classic-art"]);
        // Windows can't replace a file that is still mapped, and the app never tries to: a new
        // generation is a new file.
        drop(c);

        std::fs::write(&path, b"not a database at all, not even close to one").unwrap();
        assert!(Catalog::open(&path).unwrap_err().contains("damaged"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_catalog_from_a_newer_folderskin_says_so() {
        let conn = build::in_memory(&sample()).unwrap();
        conn.pragma_update(None, "user_version", build::CATALOG_VERSION + 1)
            .unwrap();
        let err = Catalog::from_connection(conn).err().unwrap();
        assert!(err.contains("newer FolderSkin"), "{err}");
        let other = rusqlite::Connection::open_in_memory().unwrap();
        assert!(Catalog::from_connection(other)
            .err()
            .unwrap()
            .contains("damaged"));
    }

    #[test]
    fn every_word_is_a_prefix_of_a_name_an_author_a_tag_or_a_skin() {
        let c = catalog(&sample());
        assert_eq!(
            id_set(&find(&c, "star")),
            ["classic-art", "night-prints"],
            "Starry Night and Starling"
        );
        assert_eq!(
            id_set(&find(&c, "clas")),
            ["classic-art", "greek-art"],
            "the tag"
        );
        assert_eq!(
            id_set(&find(&c, "prajwal")),
            ["classic-art", "colours"],
            "the author"
        );
        assert_eq!(
            find(&c, "star night").total,
            2,
            "both words, anywhere in a pack"
        );
        assert_eq!(ids(&find(&c, "starry toledo")), ["classic-art"]);
        assert!(
            find(&c, "starry parthenon").packs.is_empty(),
            "every word has to match"
        );
        assert_eq!(
            ids(&find(&c, "cafe")),
            ["cafe-noir"],
            "accents don't matter"
        );
        assert_eq!(ids(&find(&c, "CAFÉ")), ["cafe-noir"]);
        // FTS5 syntax typed by accident is only words.
        for odd in [
            "\"",
            "star*",
            "NOT star",
            "star OR mona",
            "(",
            "col:",
            "-",
            "a\"b",
        ] {
            let r = c.search(&Query {
                q: odd,
                limit: 5,
                ..Query::default()
            });
            assert!(r.is_ok(), "{odd:?}");
        }
        assert_eq!(find(&c, "  ").total, 5, "nothing typed lists them all");
    }

    #[test]
    fn words_in_any_script_are_split_as_the_index_splits_them() {
        // SQLite reads the vowel signs of Hindi and Thai, and the points of Hebrew and Arabic,
        // as spaces: a search has to as well, or a word becomes a phrase the skins can't be
        // searched for.
        assert_eq!(
            search::match_expr("हिन्दी").as_deref(),
            Some(r#""ह"* "न"* "द"*"#)
        );
        assert_eq!(
            search::match_expr("Café, x\u{305}y").as_deref(),
            Some(r#""Café"* "x"* "y"*"#),
            "a mark SQLite doesn't fold ends a word"
        );
        assert_eq!(
            search::match_expr("cafe\u{301}").as_deref(),
            Some("\"cafe\u{301}\"*"),
            "an accent it folds stays with its letter"
        );

        let mut packs = sample();
        packs.extend([
            pack(
                "hindi",
                "हिन्दी गीत",
                "someone",
                &["music"],
                60,
                &["किताब", "हिन्दी"],
            ),
            pack("thai", "Hello", "someone", &["hello"], 61, &["สวัสดี ครับ"]),
            pack("tamil", "தமிழ்", "someone", &["hello"], 62, &["கலை"]),
            pack("bengali", "কলকাতা", "someone", &["city"], 63, &["কলকাতা"]),
            pack("hebrew", "Peace", "someone", &["hello"], 64, &["שָׁלוֹם"]),
            pack("arabic", "Welcome", "someone", &["hello"], 65, &["مَرْحَبًا"]),
        ]);
        let c = catalog(&packs);
        for (q, id, skin) in [
            ("किताब", "hindi", "किताब"),
            ("हिन्दी", "hindi", "हिन्दी"),
            ("สวัสดี", "thai", "สวัสดี ครับ"),
            ("தமிழ்", "tamil", ""),
            ("கலை", "tamil", "கலை"),
            ("কলকাতা", "bengali", "কলকাতা"),
            ("שָׁלוֹם", "hebrew", "שָׁלוֹם"),
            ("مَرْحَبًا", "arabic", "مَرْحَبًا"),
        ] {
            let r = c
                .search(&Query {
                    q,
                    limit: 10,
                    ..Query::default()
                })
                .unwrap_or_else(|e| panic!("{q}: {e}"));
            assert!(ids(&r).contains(&id), "{q}: {:?}", ids(&r));
            if !skin.is_empty() {
                assert!(
                    r.skins.iter().any(|s| s.pack == id && s.name == skin),
                    "{q}: {:?}",
                    r.skins
                );
            }
        }
        assert_eq!(ids(&find(&c, "cafe\u{301}")), ["cafe-noir"]);
    }

    #[test]
    fn a_name_match_ranks_above_a_skin_match() {
        let c = catalog(&sample());
        // "night" names Night prints, and is only a skin of Classic Art.
        assert_eq!(ids(&find(&c, "night")), ["night-prints", "classic-art"]);
    }

    #[test]
    fn tags_narrow_the_list_and_are_counted_over_the_words_alone() {
        let c = catalog(&sample());
        let all = find(&c, "");
        assert_eq!(
            all.facets[0],
            Facet {
                tag: "classic art".into(),
                count: 2
            }
        );
        assert_eq!(all.facets.len(), 7);

        let r = c
            .search(&Query {
                q: "art",
                tag: "sculpture",
                limit: 50,
                ..Query::default()
            })
            .unwrap();
        assert_eq!(ids(&r), ["greek-art"]);
        assert_eq!(
            (r.total, r.all),
            (1, 2),
            "All still counts every pack the words match"
        );
        let counted: Vec<(&str, usize)> =
            r.facets.iter().map(|f| (f.tag.as_str(), f.count)).collect();
        assert_eq!(
            counted,
            [("classic art", 2), ("painting", 1), ("sculpture", 1)]
        );

        let browse = c
            .search(&Query {
                tag: "classic art",
                sort: Sort::Newest,
                limit: 50,
                ..Query::default()
            })
            .unwrap();
        assert_eq!(ids(&browse), ["classic-art", "greek-art"]);
        assert_eq!((browse.total, browse.all), (2, 5));

        let none = c
            .search(&Query {
                tag: "no such tag",
                limit: 50,
                ..Query::default()
            })
            .unwrap();
        assert_eq!((none.total, none.packs.len()), (0, 0));
    }

    #[test]
    fn pages_come_in_the_order_asked_for() {
        let c = catalog(&sample());
        let page = |q: &str, sort, offset, limit| {
            let r = c
                .search(&Query {
                    q,
                    sort,
                    offset,
                    limit,
                    ..Query::default()
                })
                .unwrap();
            (
                r.total,
                r.packs.into_iter().map(|p| p.id).collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            page("", Sort::Newest, 0, 2),
            (5, vec!["night-prints".into(), "classic-art".into()])
        );
        assert_eq!(
            page("", Sort::Newest, 2, 2),
            (5, vec!["greek-art".into(), "colours".into()])
        );
        assert_eq!(page("", Sort::Newest, 4, 2), (5, vec!["cafe-noir".into()]));
        assert_eq!(page("", Sort::Newest, 10, 2), (5, vec![]));
        assert_eq!(page("", Sort::Name, 0, 2).1, ["cafe-noir", "classic-art"]);
        assert_eq!(page("", Sort::Skins, 0, 1).1, ["classic-art"]);
        // With words too, and the count comes even past the end.
        assert_eq!(
            page("art", Sort::Name, 0, 1),
            (2, vec!["classic-art".into()])
        );
        assert_eq!(page("art", Sort::Name, 1, 1), (2, vec!["greek-art".into()]));
        assert_eq!(page("art", Sort::Name, 5, 1), (2, vec![]));
    }

    #[test]
    fn featured_packs_come_first_and_pages_carry_on_after_them() {
        let c = catalog(&sample());
        let featured = vec![
            "colours".to_string(),
            "gone".to_string(),
            "greek-art".to_string(),
        ];
        let page = |tag, offset, limit| {
            let r = c
                .search(&Query {
                    tag,
                    offset,
                    limit,
                    featured: &featured,
                    ..Query::default()
                })
                .unwrap();
            (
                r.total,
                r.packs.into_iter().map(|p| p.id).collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            page("", 0, 3).1,
            ["colours", "greek-art", "night-prints"],
            "featured, then the newest"
        );
        assert_eq!(
            page("", 1, 3).1,
            ["greek-art", "night-prints", "classic-art"]
        );
        assert_eq!(
            page("", 3, 3),
            (5, vec!["classic-art".into(), "cafe-noir".into()])
        );
        assert_eq!(
            page("classic art", 0, 5),
            (2, vec!["greek-art".into(), "classic-art".into()]),
            "only the featured packs with the tag"
        );
    }

    #[test]
    fn skins_that_match_come_with_their_packs_whole_words_first() {
        let c = catalog(&sample());
        let r = find(&c, "night");
        let hits: Vec<(&str, &str, usize)> = r
            .skins
            .iter()
            .map(|s| (s.pack.as_str(), s.name.as_str(), s.position))
            .collect();
        assert_eq!(
            hits,
            [
                ("classic-art", "The Starry Night", 1),
                ("night-prints", "Night heron", 1)
            ]
        );
        assert_eq!(r.skins[0].pack_name, "Classic Art");
        assert_eq!(r.skins[0].pack_hash, format!("{:016x}", 300));

        // "star" is whole in neither Starling nor Starry: both come, in the catalog's order.
        let r = find(&c, "star");
        let names: Vec<&str> = r.skins.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["The Starry Night", "Starling"]);
        // "mona lisa" whole comes before a name it only starts.
        let mut packs = sample();
        packs[1].skin_names.push("Monastery".into());
        packs[1].skin_names.push("Mona".into());
        let r = find(&catalog(&packs), "mona");
        let names: Vec<&str> = r.skins.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Mona Lisa", "Mona", "Monastery"]);

        let tagged = c
            .search(&Query {
                q: "night",
                tag: "woodblock",
                limit: 5,
                ..Query::default()
            })
            .unwrap();
        assert_eq!(tagged.skins.len(), 1);
        assert!(find(&c, "").skins.is_empty(), "nothing typed, no skins");
    }

    #[test]
    fn packs_are_found_by_id_in_the_order_asked() {
        let c = catalog(&sample());
        let want = vec!["greek-art".to_string(), "missing".into(), "colours".into()];
        let got: Vec<String> = c.packs(&want).unwrap().into_iter().map(|p| p.id).collect();
        assert_eq!(got, ["greek-art", "colours"]);
    }

    #[test]
    fn a_catalog_without_skin_names_still_searches_its_packs() {
        // What the app makes from index.json: counts, and no names.
        let mut packs = sample();
        for p in &mut packs {
            p.skin_names.clear();
        }
        let c = catalog(&packs);
        assert_eq!(c.counts(), (5, 0));
        assert_eq!(ids(&find(&c, "greek")), ["greek-art"]);
        assert!(find(&c, "mona").packs.is_empty());
        assert!(find(&c, "greek").skins.is_empty());
        assert_eq!(
            c.packs(&["classic-art".into()]).unwrap()[0].count,
            4,
            "the count is kept"
        );
    }

    /// A large made-up catalog: its size, and how long searches take. Run with
    /// `cargo test -p folderskin-catalog --release -- --ignored --nocapture scale`.
    #[test]
    #[ignore]
    fn scale() {
        let packs = synthetic(10_000, 10);
        let skins: usize = packs.iter().map(|p| p.skin_names.len()).sum();
        let t = std::time::Instant::now();
        let bytes = build::to_bytes(&packs).unwrap();
        let built = t.elapsed();
        let gz = tree::gzip(&bytes);
        println!(
            "{} packs, {skins} skins: built in {built:.2?}, {:.1} MB, {:.1} MB gzipped",
            packs.len(),
            bytes.len() as f64 / 1e6,
            gz.len() as f64 / 1e6
        );
        let dir = std::env::temp_dir().join(format!("fs-catalog-scale-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("c.sqlite");
        std::fs::write(&path, &bytes).unwrap();
        let t = std::time::Instant::now();
        let c = Catalog::open(&path).unwrap();
        println!("opened in {:.2?}", t.elapsed());
        let featured: Vec<String> = packs.iter().take(12).map(|p| p.id.clone()).collect();
        for (q, tag, sort) in [
            ("", "", Sort::Best),
            ("", "", Sort::Name),
            ("", "retro", Sort::Newest),
            ("n", "", Sort::Best),
            ("ne", "", Sort::Best),
            ("neo", "", Sort::Best),
            ("neon", "", Sort::Best),
            ("neon k", "", Sort::Best),
            ("neon koi", "", Sort::Best),
            ("mika", "", Sort::Newest),
            ("harbour", "night", Sort::Skins),
            ("zzzz", "", Sort::Best),
        ] {
            let query = Query {
                q,
                tag,
                sort,
                offset: 0,
                limit: 60,
                featured: &featured,
            };
            c.search(&query).unwrap();
            let runs = 20;
            let t = std::time::Instant::now();
            let mut r = Results::default();
            for _ in 0..runs {
                r = c.search(&query).unwrap();
            }
            println!(
                "{:>10} {:>6} {:<6?}: {:>6.2} ms  ({} packs, {} skins, {} tags)",
                format!("{q:?}"),
                tag,
                sort,
                t.elapsed().as_secs_f64() * 1000.0 / runs as f64,
                r.total,
                r.skins.len(),
                r.facets.len()
            );
        }
        let deep = Query {
            offset: 9_900,
            limit: 60,
            sort: Sort::Name,
            ..Query::default()
        };
        let t = std::time::Instant::now();
        c.search(&deep).unwrap();
        println!(
            "  a page near the end, by name: {:.2} ms",
            t.elapsed().as_secs_f64() * 1000.0
        );
        drop(c);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `n` made-up packs of about `per` skins each. Names repeat the way real ones do: a few
    /// common words and a long tail of made-up ones, so prefixes match what they would.
    fn synthetic(n: usize, per: usize) -> Vec<PackRecord> {
        const ADJ: &[&str] = &[
            "Neon", "Quiet", "Golden", "Velvet", "Paper", "Midnight", "Sunlit", "Rusty", "Pastel",
            "Electric", "Frozen", "Wild", "Tiny", "Grand", "Misty", "Lucky", "Cosmic", "Retro",
        ];
        const NOUN: &[&str] = &[
            "Koi",
            "Harbour",
            "Garden",
            "Fox",
            "Lanterns",
            "Circuits",
            "Temples",
            "Waves",
            "Owls",
            "Planets",
            "Forests",
            "Robots",
            "Mountains",
            "Cities",
            "Cats",
            "Dragons",
            "Deserts",
            "Ships",
        ];
        const SYLLABLES: &[&str] = &[
            "ka", "ri", "mo", "ne", "lu", "ta", "shi", "vo", "ra", "de", "po", "mi", "zu", "el",
            "an", "or", "ber", "lin", "gra", "fe",
        ];
        const TAGS: &[&str] = &[
            "anime",
            "retro",
            "nature",
            "space",
            "minimal",
            "pixel",
            "night",
            "animals",
            "architecture",
            "abstract",
            "photo",
            "painting",
            "cute",
            "dark",
            "pastel",
            "neon",
            "ocean",
            "food",
            "games",
            "music",
            "sport",
            "travel",
            "flowers",
            "tech",
            "vintage",
            "woodblock",
            "watercolour",
            "3d",
        ];
        const WHO: &[&str] = &[
            "prajwal-svm",
            "octocat",
            "pixel-pusher",
            "mika",
            "lena-draws",
            "tomo",
            "zed",
            "art-by-ana",
            "kofi",
            "yuki",
        ];
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = |m: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % m as u64) as usize
        };
        (0..n)
            .map(|i| {
                let name = format!("{} {}", ADJ[next(ADJ.len())], NOUN[next(NOUN.len())]);
                // Zipf-ish: a few tags on many packs, most on few.
                let tags: Vec<String> = (0..1 + next(3))
                    .map(|_| TAGS[next(TAGS.len()).min(next(TAGS.len()))].to_string())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect();
                let count = 1 + next(per * 2 - 1);
                let skin_names = (0..count)
                    .map(|_| {
                        let made_up: String = (0..2 + next(2))
                            .map(|_| SYLLABLES[next(SYLLABLES.len())])
                            .collect();
                        let mut word = made_up.chars();
                        let made_up: String = word
                            .next()
                            .map(|c| c.to_uppercase().chain(word).collect())
                            .unwrap_or_default();
                        if next(3) == 0 {
                            format!("{} {made_up}", ADJ[next(ADJ.len())])
                        } else {
                            format!("{made_up} {}", NOUN[next(NOUN.len())])
                        }
                    })
                    .collect();
                PackRecord {
                    id: format!("pack-{i:05}"),
                    name,
                    author: WHO[next(WHO.len())].into(),
                    license: "CC0-1.0".into(),
                    tags,
                    hash: format!("{i:016x}"),
                    manifest: tree::sha256_hex(format!("{i}").as_bytes()),
                    added: 1_700_000_000 + next(40_000_000) as i64,
                    count,
                    bytes: 150_000 * count as u64,
                    skin_names,
                }
            })
            .collect()
    }
}
