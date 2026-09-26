import { useEffect, useState } from "react";
import { api } from "../lib/tauri";
import { countChosen, ruledPaths, type Choice, type ChoiceCount } from "../lib/folderChoice";

/**
 * How many folders inside `folder` `choice` takes, and how many there are, as far as the
 * background count (src-tauri/src/tree/count.rs) has got: asked again each time the count grows,
 * one question at a time, until it's done. Null until the first answer.
 */
export function useChoiceCount(folder: string | null, choice: Choice | null): ChoiceCount | null {
  const [count, setCount] = useState<ChoiceCount | null>(null);
  useEffect(() => {
    setCount(null);
    if (!folder || !choice) return;
    let live = true;
    let asking = false;
    let again = false;
    const paths = ruledPaths(choice);
    const ask = () => {
      if (asking) {
        again = true;
        return;
      }
      asking = true;
      api
        .subfolderCounts(folder, paths)
        .then((counts) => {
          if (live) setCount(countChosen(choice, { inside: counts.count, done: counts.done }, counts.paths));
        })
        .catch(() => {})
        .finally(() => {
          asking = false;
          if (again && live) {
            again = false;
            ask();
          }
        });
    };
    ask();
    const stop = api.onSubfolderCount((heard) => {
      if (heard.folder === folder) ask();
    });
    return () => {
      live = false;
      stop();
    };
  }, [folder, choice]);
  return count;
}
