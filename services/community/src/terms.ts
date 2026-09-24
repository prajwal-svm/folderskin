/**
 * Why a pack was turned down or taken down, as codes a decision uses and sentences the author
 * reads. Each points at the numbered rule in docs/PACK-TERMS.md it rests on, so the app can link
 * to the rule itself.
 */

export type Reason = {
  /** The rule in docs/PACK-TERMS.md. */
  term: number;
  /** What the author reads in My submissions. */
  message: string;
  /** Turned down for what the pictures show: the same files can't simply be sent again. */
  blocks?: true;
  /** Serious enough that the computer that sent it can't share any more. */
  bans?: true;
};

export const REASONS: Record<string, Reason> = {
  "not-yours": { term: 1, message: "It wasn't clear the pictures are yours to share." },
  taken: {
    term: 2,
    message: "Some of the pictures seem to come from somewhere else, such as another app, a game or a stock library.",
  },
  "ai-terms": { term: 3, message: "The pictures copy an existing character or a living artist's style." },
  credit: { term: 4, message: "The name on the pack isn't yours to use." },
  licence: { term: 5, message: "The licence chosen doesn't fit the pictures." },
  sexual: { term: 6, message: "The pictures include sexual or suggestive content.", blocks: true },
  minor: { term: 7, message: "The pictures sexualise a child.", blocks: true, bans: true },
  "self-harm": { term: 8, message: "The pictures depict or encourage self-harm.", blocks: true },
  hate: { term: 9, message: "The pictures include hateful content or symbols.", blocks: true },
  gore: { term: 10, message: "The pictures include gore or shocking violence.", blocks: true },
  harassment: { term: 11, message: "The pictures target a real person or show someone without their permission.", blocks: true },
  illegal: { term: 12, message: "The pictures promote something illegal.", blocks: true },
  brand: { term: 13, message: "The pictures use someone else's logo, trade mark or characters." },
  deceptive: { term: 14, message: "The files aren't only pictures, or they try to mislead the checks.", blocks: true, bans: true },
  quality: { term: 15, message: "The pictures don't work well enough as folder icons yet." },
  duplicate: { term: 15, message: "It's very close to a pack that's already shared." },
  fit: { term: 15, message: "It isn't a good fit for FolderSkin's community packs." },
};

export const isReason = (code: unknown): code is string => typeof code === "string" && Object.hasOwn(REASONS, code);

/** The reasons as the author sees them: the code, the rule and the sentence. */
export function explain(codes: string[]): { code: string; term: number; message: string }[] {
  return codes.filter(isReason).map((code) => ({ code, term: REASONS[code].term, message: REASONS[code].message }));
}
