import { useRef, useState, type KeyboardEvent } from "react";
import { cleanTags, MAX_TAG_CHARS, MAX_TAGS } from "../lib/tags";

/**
 * Tags as chips with a field for more. Return or a comma adds what was typed, Backspace in the
 * empty field takes the last one off, and tags already used elsewhere are one click away. Return
 * in the empty field is left alone, so the form around it submits.
 */
export function TagInput({
  value,
  onChange,
  suggestions = [],
  max = MAX_TAGS,
  label,
}: {
  value: string[];
  onChange: (tags: string[]) => void;
  /** Tags to offer, most useful first. Ones already chosen are left out. */
  suggestions?: string[];
  max?: number;
  label: string;
}) {
  const [draft, setDraft] = useState("");
  const input = useRef<HTMLInputElement>(null);
  const full = value.length >= max;
  const offered = suggestions.filter((t) => !value.includes(t)).slice(0, 8);

  const add = (...typed: string[]) => {
    onChange(cleanTags([...value, ...typed], max));
    setDraft("");
  };

  const keys = (e: KeyboardEvent<HTMLInputElement>) => {
    if ((e.key === "Enter" || e.key === ",") && draft.trim()) {
      e.preventDefault();
      add(draft);
    } else if (e.key === "Backspace" && !draft && value.length > 0) {
      onChange(value.slice(0, -1));
    }
  };

  return (
    <div className="tags-field">
      <div className="tag-input" onMouseDown={(e) => e.target === e.currentTarget && (e.preventDefault(), input.current?.focus())}>
        {value.map((tag) => (
          <span className="tag-chip" key={tag}>
            {tag}
            <button
              type="button"
              className="tag-chip-x"
              aria-label={`remove the tag ${tag}`}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => onChange(value.filter((t) => t !== tag))}
            >
              <svg width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" aria-hidden="true">
                <path d="M18 6 6 18M6 6l12 12" />
              </svg>
            </button>
          </span>
        ))}
        {!full && (
          <input
            ref={input}
            className="tag-input-field"
            value={draft}
            aria-label={label}
            placeholder={value.length ? "Add another" : "Add a tag"}
            maxLength={MAX_TAG_CHARS + 8}
            spellCheck={false}
            autoComplete="off"
            onChange={(e) => (e.target.value.includes(",") ? add(...e.target.value.split(",")) : setDraft(e.target.value))}
            onKeyDown={keys}
            onBlur={() => draft.trim() && add(draft)}
          />
        )}
      </div>
      {full ? (
        <p className="field-note">{max} tags is the most.</p>
      ) : (
        offered.length > 0 && (
          <div className="tag-suggest">
            {offered.map((tag) => (
              <button type="button" key={tag} className="tag-suggest-btn" onMouseDown={(e) => e.preventDefault()} onClick={() => add(tag)}>
                + {tag}
              </button>
            ))}
          </div>
        )
      )}
    </div>
  );
}
