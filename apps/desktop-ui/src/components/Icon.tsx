import type { SVGProps } from "react";

export type IconName =
  | "archive"
  | "alert"
  | "check"
  | "chevron"
  | "clipboard"
  | "close"
  | "export"
  | "file"
  | "filter"
  | "folder"
  | "globe"
  | "history"
  | "home"
  | "info"
  | "menu"
  | "more"
  | "plus"
  | "search"
  | "settings"
  | "shield"
  | "sparkles"
  | "upload";

export interface IconProps extends Omit<SVGProps<SVGSVGElement>, "children"> {
  name: IconName;
  size?: number;
}

function paths(name: IconName) {
  switch (name) {
    case "archive":
      return (
        <>
          <path d="M4 7h16" />
          <path d="M5 7l1 13h12l1-13" />
          <path d="M8 4h8l1 3H7l1-3Z" />
          <path d="M9 11h6" />
        </>
      );
    case "alert":
      return (
        <>
          <path d="M12 3 2.8 20h18.4L12 3Z" />
          <path d="M12 9v5" />
          <path d="M12 17.5v.1" />
        </>
      );
    case "check":
      return (
        <>
          <circle cx="12" cy="12" r="9" />
          <path d="m8 12 2.5 2.5L16.5 8.5" />
        </>
      );
    case "chevron":
      return <path d="m9 6 6 6-6 6" />;
    case "clipboard":
      return (
        <>
          <rect x="5" y="4" width="14" height="17" rx="2" />
          <path d="M9 4V2h6v2" />
          <path d="M9 9h6M9 13h6M9 17h4" />
        </>
      );
    case "close":
      return <path d="m6 6 12 12M18 6 6 18" />;
    case "export":
      return (
        <>
          <path d="M12 3v12" />
          <path d="m7.5 7.5 4.5-4.5 4.5 4.5" />
          <path d="M5 13v7h14v-7" />
        </>
      );
    case "file":
      return (
        <>
          <path d="M6 2h8l4 4v16H6V2Z" />
          <path d="M14 2v5h5" />
          <path d="M9 12h6M9 16h6" />
        </>
      );
    case "filter":
      return (
        <>
          <path d="M4 5h16" />
          <path d="M7 12h10" />
          <path d="M10 19h4" />
        </>
      );
    case "folder":
      return <path d="M3 6h7l2 2h9v12H3V6Z" />;
    case "globe":
      return (
        <>
          <circle cx="12" cy="12" r="9" />
          <path d="M3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18" />
        </>
      );
    case "history":
      return (
        <>
          <path d="M4 8V3m0 0h5M4 3l3 3" />
          <path d="M4.5 8A9 9 0 1 1 3 14" />
          <path d="M12 7v5l3 2" />
        </>
      );
    case "home":
      return (
        <>
          <path d="m3 11 9-8 9 8" />
          <path d="M5 10v11h14V10M9 21v-7h6v7" />
        </>
      );
    case "info":
      return (
        <>
          <circle cx="12" cy="12" r="9" />
          <path d="M12 11v6M12 7v.1" />
        </>
      );
    case "menu":
      return (
        <>
          <path d="M4 7h16" />
          <path d="M4 12h16" />
          <path d="M4 17h16" />
        </>
      );
    case "more":
      return (
        <>
          <circle cx="5" cy="12" r="1" fill="currentColor" stroke="none" />
          <circle cx="12" cy="12" r="1" fill="currentColor" stroke="none" />
          <circle cx="19" cy="12" r="1" fill="currentColor" stroke="none" />
        </>
      );
    case "plus":
      return <path d="M12 4v16M4 12h16" />;
    case "search":
      return (
        <>
          <circle cx="10.5" cy="10.5" r="7" />
          <path d="m16 16 5 5" />
        </>
      );
    case "settings":
      return (
        <>
          <circle cx="12" cy="12" r="3" />
          <path
            d="M19 13.5v-3l-2.2-.7-.8-1.9 1-2-2.1-2.1-2 1-1.9-.8L10.5 2h-3l-.7 2.2-1.9.8-2-1L.8 6.1l1 2-.8 1.9-2.2.7v3l2.2.7.8 1.9-1 2 2.1 2.1 2-1 1.9.8.7 2.2h3l.7-2.2 1.9-.8 2 1 2.1-2.1-1-2 .8-1.9 2.2-.7Z"
            transform="translate(2.5 -.2) scale(.8)"
          />
        </>
      );
    case "shield":
      return (
        <>
          <path d="M12 2 20 5v6c0 5-3.2 8.8-8 11-4.8-2.2-8-6-8-11V5l8-3Z" />
          <path d="m8.5 12 2.2 2.2 4.8-5" />
        </>
      );
    case "sparkles":
      return (
        <>
          <path d="m8 3 1 3 3 1-3 1-1 3-1-3-3-1 3-1 1-3Z" />
          <path d="m16 12 1.4 4.6L22 18l-4.6 1.4L16 24l-1.4-4.6L10 18l4.6-1.4L16 12Z" />
        </>
      );
    case "upload":
      return (
        <>
          <path d="M12 16V3" />
          <path d="m7 8 5-5 5 5" />
          <path d="M4 13v8h16v-8" />
        </>
      );
  }
}

/** Consistent 1.75px outline icon used throughout application chrome. */
export function Icon({ name, size = 20, ...props }: IconProps) {
  const renderedSize = size * 0.75;

  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 24 24"
      width={renderedSize}
      height={renderedSize}
      fill="none"
      stroke="currentColor"
      strokeWidth="1.75"
      strokeLinecap="round"
      strokeLinejoin="round"
      {...props}
    >
      {paths(name)}
    </svg>
  );
}
