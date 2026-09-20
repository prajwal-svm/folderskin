import { useState } from "react";
import type { GithubAccount } from "../lib/tauri";

/** Their GitHub picture, or their initial when it can't be fetched. */
export function GithubAvatar({ account }: { account: GithubAccount }) {
  const [failed, setFailed] = useState(false);
  if (!account.avatar_url || failed) {
    return (
      <span className="gh-avatar is-letter" aria-hidden="true">
        {account.login.charAt(0).toUpperCase()}
      </span>
    );
  }
  return <img className="gh-avatar" src={account.avatar_url} alt="" draggable={false} onError={() => setFailed(true)} />;
}
