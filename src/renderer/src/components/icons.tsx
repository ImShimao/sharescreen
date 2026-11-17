import type { SVGProps } from 'react';

type IconProps = SVGProps<SVGSVGElement>;

const base = (props: IconProps): IconProps => ({
  width: 18,
  height: 18,
  viewBox: '0 0 24 24',
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.7,
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
  'aria-hidden': true,
  ...props,
});

export const IconBack = (p: IconProps) => (
  <svg {...base(p)}>
    <path d="M15 6l-6 6 6 6" />
  </svg>
);

export const IconHome = (p: IconProps) => (
  <svg {...base(p)}>
    <circle cx="12" cy="12" r="7.5" />
  </svg>
);

export const IconRecents = (p: IconProps) => (
  <svg {...base(p)}>
    <rect x="4" y="5" width="11" height="14" rx="2.2" />
    <path d="M18 8.5v8.2a2 2 0 0 1-1.6 2" />
  </svg>
);

export const IconVolumeUp = (p: IconProps) => (
  <svg {...base(p)}>
    <path d="M4 10v4h3.5L12 18V6L7.5 10H4z" />
    <path d="M15.5 9a4 4 0 0 1 0 6M18.5 6.5a7.5 7.5 0 0 1 0 11" />
  </svg>
);

export const IconVolumeDown = (p: IconProps) => (
  <svg {...base(p)}>
    <path d="M4 10v4h3.5L12 18V6L7.5 10H4z" />
    <path d="M15.5 9a4 4 0 0 1 0 6" />
  </svg>
);

export const IconSound = (p: IconProps) => (
  <svg {...base(p)}>
    <path d="M4 10v4h3.5L12 18V6L7.5 10H4z" />
    <path d="M15.5 9a4 4 0 0 1 0 6M18.5 6.5a7.5 7.5 0 0 1 0 11" />
  </svg>
);

export const IconMuted = (p: IconProps) => (
  <svg {...base(p)}>
    <path d="M4 10v4h3.5L12 18V6L7.5 10H4z" />
    <path d="M16 9.5l5 5M21 9.5l-5 5" />
  </svg>
);

export const IconRotate = (p: IconProps) => (
  <svg {...base(p)}>
    <rect x="7" y="3.5" width="10" height="17" rx="2.2" />
    <path d="M3.5 9.5a8 8 0 0 1 3-2.6M20.5 14.5a8 8 0 0 1-3 2.6" />
  </svg>
);

export const IconCamera = (p: IconProps) => (
  <svg {...base(p)}>
    <path d="M4 8.5A2.5 2.5 0 0 1 6.5 6h1.7l1.4-2h2.8l1.4 2h1.7A2.5 2.5 0 0 1 20 8.5v8A2.5 2.5 0 0 1 17.5 19h-11A2.5 2.5 0 0 1 4 16.5z" />
    <circle cx="12" cy="12.5" r="3.5" />
  </svg>
);

export const IconRecord = (p: IconProps) => (
  <svg {...base(p)}>
    <circle cx="12" cy="12" r="4.2" fill="currentColor" stroke="none" />
    <circle cx="12" cy="12" r="8" />
  </svg>
);

export const IconStop = (p: IconProps) => (
  <svg {...base(p)}>
    <rect x="7" y="7" width="10" height="10" rx="2" fill="currentColor" stroke="none" />
  </svg>
);

export const IconFiles = (p: IconProps) => (
  <svg {...base(p)}>
    <path d="M12 15V4M7.5 8.5L12 4l4.5 4.5" />
    <path d="M4 14.5V18a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3.5" />
  </svg>
);

export const IconRefresh = (p: IconProps) => (
  <svg {...base(p)}>
    <path d="M20 11a8 8 0 1 0-2.3 5.7M20 4.5V11h-6.5" />
  </svg>
);

export const IconTablet = (p: IconProps) => (
  <svg {...base(p)}>
    <rect x="3" y="5.5" width="18" height="13" rx="2.5" />
    <path d="M10.5 15.5h3" />
  </svg>
);

export const IconPhone = (p: IconProps) => (
  <svg {...base(p)}>
    <rect x="6.5" y="3" width="11" height="18" rx="2.6" />
    <path d="M10.5 18h3" />
  </svg>
);

export const IconWarn = (p: IconProps) => (
  <svg {...base(p)}>
    <path d="M12 4l9 16H3z" />
    <path d="M12 10v4.5M12 17.2v.1" />
  </svg>
);

export const IconCheck = (p: IconProps) => (
  <svg {...base(p)}>
    <path d="M5 12.5l4.2 4.2L19 7" />
  </svg>
);

export const IconDot = (p: IconProps) => (
  <svg {...base(p)}>
    <circle cx="12" cy="12" r="4" fill="currentColor" stroke="none" />
  </svg>
);
