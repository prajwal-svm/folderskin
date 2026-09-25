import { Fragment, type ReactNode } from "react";
import { interpolate, richParts, type Vars } from "./core";
import { INTL_LOCALES, useLocale, useT, type MessageKey } from "./index";

/**
 * A message with markup in it, for the few sentences that put a word in bold or a link in the
 * middle: `"Credited to <b>{{handle}}</b>."` with `tags={{ b: (text) => <strong>{text}</strong> }}`.
 * The tags are read before the placeholders are filled, so a name with "<" in it stays text. A tag
 * with no renderer shows its text as it is. `text` draws the plain parts, such as `branded` for a
 * sentence with the app's name in it.
 */
export function Rich({
  k,
  vars,
  tags = {},
  text = (s) => s,
}: {
  k: MessageKey;
  vars?: Vars;
  tags?: Record<string, (text: string) => ReactNode>;
  text?: (text: string) => ReactNode;
}) {
  const t = useT();
  const intl = INTL_LOCALES[useLocale()];
  // Only the count goes in first: it picks the plural. Everything else fills each piece after.
  const message = t(k, typeof vars?.count === "number" ? { count: vars.count } : undefined);
  return (
    <>
      {richParts(message).map((part, i) => {
        const filled = interpolate(part.text, vars, intl);
        if (!("tag" in part)) return <Fragment key={i}>{text(filled)}</Fragment>;
        const render = tags[part.tag];
        return <Fragment key={i}>{render ? render(filled) : text(filled)}</Fragment>;
      })}
    </>
  );
}
