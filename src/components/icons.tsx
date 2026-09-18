/** Small line icons, 1.8 px strokes, drawn to match the app's flat black-and-green look. */

type P = { size?: number };

const base = (size: number) => ({
  viewBox: "0 0 24 24",
  width: size,
  height: size,
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.9,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  "aria-hidden": true,
});

export const IconImage = ({ size = 18 }: P) => (
  <svg {...base(size)}>
    <rect x="3.5" y="4.5" width="17" height="15" rx="3" />
    <circle cx="9" cy="9.5" r="1.6" fill="currentColor" stroke="none" />
    <path d="M5 17.5l4.6-4.6a1.2 1.2 0 0 1 1.7 0l2.4 2.4 1.9-1.9a1.2 1.2 0 0 1 1.7 0L20 16" />
  </svg>
);

export const IconSliders = ({ size = 18 }: P) => (
  <svg {...base(size)}>
    <path d="M4 7h10M18 7h2M4 17h4M12 17h8" />
    <circle cx="16" cy="7" r="2.2" />
    <circle cx="10" cy="17" r="2.2" />
  </svg>
);

export const IconStar = ({ size = 14, filled = false }: P & { filled?: boolean }) => (
  <svg {...base(size)} fill={filled ? "currentColor" : "none"}>
    <path d="M12 3.6l2.6 5.4 5.9.8-4.3 4.1 1.1 5.9L12 17l-5.3 2.8 1.1-5.9-4.3-4.1 5.9-.8z" />
  </svg>
);

export const IconCheck = ({ size = 14 }: P) => (
  <svg {...base(size)} strokeWidth={2.4}>
    <path d="M5 12.5l4.2 4.2L19 7" />
  </svg>
);

export const IconArrowDown = ({ size = 14 }: P) => (
  <svg {...base(size)} strokeWidth={2.4}>
    <path d="M12 5v14M6 13l6 6 6-6" />
  </svg>
);

export const IconUndo = ({ size = 15 }: P) => (
  <svg {...base(size)}>
    <path d="M4 12a8 8 0 1 0 2.6-5.9" />
    <path d="M3.5 4v5h5" />
  </svg>
);

export const IconFolder = ({ size = 22 }: P) => (
  <svg {...base(size)}>
    <path d="M3.5 7.5a2 2 0 0 1 2-2h4.2l2 2.2h6.8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z" />
  </svg>
);

export const IconSpinner = ({ size = 14 }: P) => (
  <svg {...base(size)} strokeWidth={2.4} className="spin">
    <path d="M12 4a8 8 0 1 1-8 8" />
  </svg>
);

export const IconGrid = ({ size = 20 }: P) => (
  <svg {...base(size)}>
    <rect x="3.5" y="3.5" width="7" height="7" rx="2" />
    <rect x="13.5" y="3.5" width="7" height="7" rx="2" />
    <rect x="3.5" y="13.5" width="7" height="7" rx="2" />
    <rect x="13.5" y="13.5" width="7" height="7" rx="2" />
  </svg>
);

export const IconGlobe = ({ size = 20 }: P) => (
  <svg {...base(size)}>
    <circle cx="12" cy="12" r="8.5" />
    <path d="M3.5 12h17M12 3.5c2.6 2.6 2.6 14.4 0 17M12 3.5c-2.6 2.6-2.6 14.4 0 17" />
  </svg>
);

export const IconSparkles = ({ size = 20 }: P) => (
  <svg {...base(size)}>
    <path d="M12 3.5l1.9 5.1 5.1 1.9-5.1 1.9L12 17.5l-1.9-5.1L5 10.5l5.1-1.9z" />
    <path d="M19 15.5l.8 2.2 2.2.8-2.2.8-.8 2.2-.8-2.2-2.2-.8 2.2-.8z" />
  </svg>
);

export const IconSun = ({ size = 19 }: P) => (
  <svg {...base(size)}>
    <circle cx="12" cy="12" r="4" />
    <path d="M12 2.5v2.5M12 19v2.5M2.5 12H5M19 12h2.5M5.3 5.3l1.8 1.8M16.9 16.9l1.8 1.8M5.3 18.7l1.8-1.8M16.9 7.1l1.8-1.8" />
  </svg>
);

export const IconMoon = ({ size = 19 }: P) => (
  <svg {...base(size)}>
    <path d="M20 14.5A8.5 8.5 0 0 1 9.5 4a8.5 8.5 0 1 0 10.5 10.5z" />
  </svg>
);

export const IconSearch = ({ size = 16 }: P) => (
  <svg {...base(size)}>
    <circle cx="11" cy="11" r="6.5" />
    <path d="M16 16l4.5 4.5" />
  </svg>
);

export const IconDownload = ({ size = 18 }: P) => (
  <svg {...base(size)}>
    <path d="M12 4v11M7 10.5l5 5 5-5M4.5 19.5h15" />
  </svg>
);

export const IconExternal = ({ size = 15 }: P) => (
  <svg {...base(size)}>
    <path d="M14 4.5h5.5V10M19.5 4.5L11 13M9 6H6.5a2 2 0 0 0-2 2v9.5a2 2 0 0 0 2 2H16a2 2 0 0 0 2-2V15" />
  </svg>
);
