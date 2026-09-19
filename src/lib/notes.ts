/**
 * A release's notes as the update dialog draws them. They are the version's section of
 * CHANGELOG.md, so only what that file uses is understood: "###" headings, "- " lists whose items
 * wrap onto indented lines, paragraphs, and **bold**, `code` and [links](…) inside a line.
 */

export type NotesBlock = { kind: "heading"; text: string } | { kind: "list"; items: string[] } | { kind: "text"; text: string };

export type Inline = { kind: "text" | "strong" | "code"; text: string };

/**
 * Splits notes into blocks. Everything from a "---" line on is the download list the release
 * page adds, which doesn't belong in the app.
 */
export function parseNotes(markdown: string): NotesBlock[] {
  const blocks: NotesBlock[] = [];
  let text: string[] = [];
  let list: string[] | null = null;
  const endText = () => {
    if (text.length) blocks.push({ kind: "text", text: text.join(" ") });
    text = [];
  };
  const endList = () => {
    if (list) blocks.push({ kind: "list", items: list });
    list = null;
  };

  for (const raw of markdown.replace(/\r\n?/g, "\n").split("\n")) {
    const line = raw.trim();
    if (/^-{3,}$/.test(line)) break;
    if (!line) {
      endText();
      endList();
      continue;
    }
    const heading = /^#{1,6}\s+(.+)$/.exec(line);
    if (heading) {
      endText();
      endList();
      blocks.push({ kind: "heading", text: heading[1] });
      continue;
    }
    const bullet = /^[-*]\s+(.+)$/.exec(line);
    if (bullet) {
      endText();
      (list ??= []).push(bullet[1]);
      continue;
    }
    // An indented line carries on the item above it.
    if (list && /^\s/.test(raw)) {
      list[list.length - 1] += ` ${line}`;
      continue;
    }
    endList();
    text.push(line);
  }
  endText();
  endList();
  return blocks;
}

const INLINE = /\*\*(.+?)\*\*|`([^`]+)`|\[([^\]]+)\]\([^)\s]*\)/g;

/** A line's bold and code, with a link kept as its words. */
export function splitInline(line: string): Inline[] {
  const parts: Inline[] = [];
  let at = 0;
  for (const m of line.matchAll(INLINE)) {
    if (m.index > at) parts.push({ kind: "text", text: line.slice(at, m.index) });
    if (m[1] !== undefined) parts.push({ kind: "strong", text: m[1] });
    else if (m[2] !== undefined) parts.push({ kind: "code", text: m[2] });
    else parts.push({ kind: "text", text: m[3] });
    at = m.index + m[0].length;
  }
  if (at < line.length) parts.push({ kind: "text", text: line.slice(at) });
  return parts;
}
