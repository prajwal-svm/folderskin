/** Header lockup: the app icon beside the stacked product name. */
export function Wordmark() {
  return (
    <div className="brand-lockup" aria-label="FolderSkin">
      <img className="brand-icon" src="/app-icon.png" alt="" draggable={false} />
      <p className="brand-name">
        Folder
        <br />
        Skin
      </p>
    </div>
  );
}
