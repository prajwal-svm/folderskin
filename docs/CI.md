# Continuous integration

| Workflow | Runs on | What it does |
| --- | --- | --- |
| **CI** (`ci.yml`) | pushes and pull requests to `main`, except ones that only touch docs or `community/` | the jobs below |
| **Community** (`community.yml`) | pull requests that touch `community/`, pushes to `main` that change a pack | checks every pack; on `main`, rebuilds `community/index.json` and the previews |
| **CodeQL** (`codeql.yml`) | pushes and pull requests to `main`, once the repository is public | code scanning for TypeScript and the workflows |
| **Release** (`release.yml`) | a `v*.*.*` tag, or by hand | draft release with installers for every platform ([RELEASING.md](RELEASING.md)) |
| **Dependabot** (`.github/dependabot.yml`) | Mondays | one pull request of minor and patch updates per ecosystem |

The CI jobs:

| Job | Runner | Checks |
| --- | --- | --- |
| Frontend | Ubuntu | `pnpm audit` of production dependencies, `pnpm build` (tsc, then Vite), `pnpm test:coverage` |
| Rust | Ubuntu | `cargo fmt --check`, `cargo clippy -D warnings`, `cargo llvm-cov` over the workspace's tests, `packs check` on the community packs |
| Rust (macOS), Rust (Windows) | macOS, Windows | clippy and tests again, because the code that writes icons only compiles on its own system |
| cargo-deny | Ubuntu | RustSec advisories, and licences an MIT app can ship (`deny.toml`) |
| SonarQube Cloud | Ubuntu | static analysis and coverage, once it's set up (below) |
| Bundle | all three | the installers, unsigned, kept for 14 days to try a change on a system you don't have |

Run the same checks locally with:

```sh
pnpm typecheck && pnpm test
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny --workspace --all-features check
```

## The Rust version

`rust-toolchain.toml` pins one Rust version for everything: rustup reads it for any `cargo` call
inside the repository, and `.github/actions/rust-toolchain` installs the same version on the
runners. To move to a newer Rust, change `channel`, run the three cargo checks above, and fix what
the new clippy finds in the same commit. With a floating `stable`, a clippy release once failed
CI on code nobody had touched.

## SonarQube Cloud

Sonar reads the TypeScript and the Rust for bugs, code smells, security hotspots and duplicated
code, and tracks test coverage. Its quality gate and coverage badges are at the top of the
README. It's worth having here mainly for the code that handles what comes from outside: pictures
people drop on the window, community packs downloaded from GitHub, and answers from AI providers.
Clippy and the type checker don't look for security hotspots.

The job is skipped until the project exists. To set it up once:

1. Sign in at [sonarcloud.io](https://sonarcloud.io) with GitHub and add an organisation for the
   `prajwal-svm` account. The `oleafly` organisation is tied to the Oleafly GitHub organisation,
   so it can't import a repository from a personal account.
2. **Analyze new project** → `folderskin`. Keep the key `prajwal-svm_folderskin`; the README
   badges use it.
3. In the project's **Administration → Analysis Method**, turn **Automatic Analysis** off. CI
   runs the analysis, with the coverage reports, and the two can't both be on.
4. Give the repository the settings the job reads:

   ```sh
   gh variable set SONAR_ORGANIZATION --repo prajwal-svm/folderskin --body prajwal-svm
   ```

   ```sh
   gh variable set SONAR_PROJECT_KEY --repo prajwal-svm/folderskin --body prajwal-svm_folderskin
   ```

   ```sh
   gh secret set SONAR_TOKEN --repo prajwal-svm/folderskin
   ```

   The token Oleafly uses works here too if it's a personal token (**My Account → Security**).
   A token scoped to the `oleafly` organisation doesn't.

The free plan covers public projects without limit, and private ones up to 50,000 lines.
FolderSkin is about 15,500.

What counts towards coverage is set in `sonar-project.properties`: the logic in `src/lib`,
`src/state` and the Rust crates. The React components have no component tests yet, and the macOS
and Windows code only runs in jobs that report no coverage, so those are left out of the
percentage. Sonar still analyses them.

## Actions minutes while the repository is private

Public repositories run Actions for free. A private one spends the account's monthly minutes,
and macOS minutes count ten times and Windows twice, so one full CI run can use a few hundred.
So while the repository is private, a pull request runs only the Linux jobs (frontend, Rust,
cargo-deny, Sonar) and skips the macOS and Windows checks and the bundles; pushes to `main` run
everything. Once the repository is public, pull requests get the full set again without any
change. Docs-only pushes skip CI for the same reason.
