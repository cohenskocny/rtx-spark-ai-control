import type { ReactNode } from "react";
import { Glyph } from "./primitives";

export interface NavItem {
  id: string;
  label: string;
  glyph: string;
}

export const NAV: NavItem[] = [
  { id: "home", label: "Home", glyph: "home" },
  { id: "server", label: "Server", glyph: "server" },
  { id: "blackout", label: "Blackout", glyph: "blackout" },
  { id: "updates", label: "Updates", glyph: "updates" },
  { id: "setup", label: "Setup", glyph: "setup" },
  { id: "settings", label: "Settings", glyph: "settings" },
  { id: "logs", label: "Logs", glyph: "logs" },
];

/** The application mark: a four-point spark cut from cobalt glass. Same geometry
 *  as `src-tauri/icons` (see tools/make-icon.py), drawn inline so the rail scales
 *  with the interface. */
export function BrandMark() {
  return (
    <span className="brand-mark" aria-hidden="true">
      <svg viewBox="0 0 24 24" role="presentation">
        <defs>
          <linearGradient id="brand-tile" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="#3a60e2" />
            <stop offset="55%" stopColor="#1a3094" />
            <stop offset="100%" stopColor="#091136" />
          </linearGradient>
          <linearGradient id="brand-sheen" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="#d6e8ff" stopOpacity="0.32" />
            <stop offset="100%" stopColor="#d6e8ff" stopOpacity="0" />
          </linearGradient>
          <radialGradient id="brand-spark" cx="36%" cy="30%" r="74%">
            <stop offset="0%" stopColor="#f8fbff" />
            <stop offset="24%" stopColor="#c4deff" />
            <stop offset="62%" stopColor="#80aaff" />
            <stop offset="100%" stopColor="#2c52c8" />
          </radialGradient>
        </defs>
        <rect x="0.8" y="0.8" width="22.4" height="22.4" rx="5.6" fill="url(#brand-tile)" />
        <rect x="0.8" y="0.8" width="22.4" height="22.4" rx="5.6" fill="url(#brand-sheen)" />
        <polygon
          points="12,2.74 13.9,10.1 21.26,12 13.9,13.9 12,21.26 10.1,13.9 2.74,12 10.1,10.1"
          fill="url(#brand-spark)"
        />
        <g aria-hidden="true">
          <path d="M12 12 12 2.74 13.9 10.1Z" fill="#ffffff" opacity="0.22" />
          <path d="M12 12 12 21.26 10.1 13.9Z" fill="#ffffff" opacity="0.2" />
          <path d="M12 12 2.74 12 10.1 10.1Z" fill="#ffffff" opacity="0.1" />
          <path d="M12 12 13.9 10.1 21.26 12Z" fill="#0a1740" opacity="0.16" />
          <path d="M12 12 10.1 13.9 2.74 12Z" fill="#0a1740" opacity="0.16" />
          <path d="M12 12 13.9 13.9 12 21.26Z" fill="#0a1740" opacity="0.12" />
          <path d="M12 12 10.1 10.1 12 2.74Z" fill="#0a1740" opacity="0.1" />
        </g>
        <rect
          x="0.8"
          y="0.8"
          width="22.4"
          height="22.4"
          rx="5.6"
          fill="none"
          stroke="#96befe"
          strokeOpacity="0.26"
          strokeWidth="0.6"
        />
      </svg>
    </span>
  );
}

export function Shell({
  page,
  onNavigate,
  topRight,
  foot,
  children,
}: {
  page: string;
  onNavigate: (id: string) => void;
  topRight: ReactNode;
  foot: ReactNode;
  children: ReactNode;
}) {
  const current = NAV.find((item) => item.id === page) ?? NAV[0];
  return (
    <>
      <div className="field" />
      <div className="aurora" />
      <div className="shell">
        <aside className="glass rail">
          <div className="brand">
            <BrandMark />
            <div className="brand-text">
              <span className="name">RTX Spark</span>
              <span className="sub">AI Control · V2</span>
            </div>
          </div>

          <nav className="nav" aria-label="Primary">
            {NAV.map((item) => (
              <button
                key={item.id}
                type="button"
                className="nav-item"
                data-active={item.id === page}
                onClick={() => onNavigate(item.id)}
              >
                <Glyph name={item.glyph} />
                <span>{item.label}</span>
              </button>
            ))}
          </nav>

          <div className="rail-spacer" />
          <div className="rail-foot">{foot}</div>
        </aside>

        <div className="main">
          <header className="topbar">
            <div className="topbar-left">
              <span className="crumb">Workspace</span>
              <span className="pill ghost sm">{current.label}</span>
            </div>
            <div className="topbar-right">{topRight}</div>
          </header>
          <div className="page" key={page}>
            <div className="page-enter">{children}</div>
          </div>
        </div>
      </div>
    </>
  );
}
