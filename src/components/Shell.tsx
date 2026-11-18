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
  { id: "settings", label: "Settings", glyph: "settings" },
  { id: "logs", label: "Logs", glyph: "logs" },
];

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
            <div className="brand-mark" />
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
