export function TabBar({ tabs, active, onChange }: { tabs: string[]; active: string; onChange: (tab: string) => void }) {
  return (
    <nav className="tabbar" role="tablist" aria-label="skin collections">
      {tabs.map((tab) => (
        <button
          key={tab}
          role="tab"
          type="button"
          aria-selected={tab === active}
          className={tab === active ? "tab is-active" : "tab"}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => onChange(tab)}
        >
          {tab}
        </button>
      ))}
    </nav>
  );
}
