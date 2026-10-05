// Componentes de la base de diseño (docs/app.md, "Diseño"): iconos, avisos, cabeceras y cifras.
import { ReactNode } from "react";

interface IconProps {
  size?: number;
}

function Svg({ size = 18, children }: IconProps & { children: ReactNode }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

/** Baliza de orientación: cuadrado partido en diagonal, blanco y naranja. */
export function ControlFlag({ size = 22 }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
      <rect x="2.5" y="2.5" width="19" height="19" rx="2" fill="#ffffff" stroke="#d4580b" />
      <path d="M21.5 2.5 L21.5 19.5 Q21.5 21.5 19.5 21.5 L2.5 21.5 Z" fill="#e8680f" />
    </svg>
  );
}

export function ListIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M9 6h11M9 12h11M9 18h11" />
      <circle cx="4.5" cy="6" r="1" />
      <circle cx="4.5" cy="12" r="1" />
      <circle cx="4.5" cy="18" r="1" />
    </Svg>
  );
}

/** Columnas: el histórico. */
export function ChartIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M4 20h16" />
      <path d="M7 16v-5M12 16V6M17 16v-8" />
    </Svg>
  );
}

export function UploadIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M12 15V4M7.5 8.5 12 4l4.5 4.5" />
      <path d="M4 15v3.5A1.5 1.5 0 0 0 5.5 20h13a1.5 1.5 0 0 0 1.5-1.5V15" />
    </Svg>
  );
}

export function SlidersIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M4 7h9M17 7h3M4 17h3M11 17h9" />
      <circle cx="15" cy="7" r="2" />
      <circle cx="9" cy="17" r="2" />
    </Svg>
  );
}

export function FileIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" />
      <path d="M14 3v5h5" />
    </Svg>
  );
}

export function WatchIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <rect x="6" y="6" width="12" height="12" rx="3" />
      <path d="M9 6l.5-3h5l.5 3M9 18l.5 3h5l.5-3M12 9.5V12l1.5 1.5" />
    </Svg>
  );
}

export function ChevronRight(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M9 6l6 6-6 6" />
    </Svg>
  );
}

export function ChevronLeft(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M15 6l-6 6 6 6" />
    </Svg>
  );
}

export function CloseIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M6 6l12 12M18 6L6 18" />
    </Svg>
  );
}

export function PencilIcon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M4 20h4L19 9l-4-4L4 16v4zM13.5 6.5l4 4" />
    </Svg>
  );
}

type NoticeKind = "info" | "warning" | "error" | "success";

/** Aviso con color según su tipo. Los de error se anuncian a los lectores de pantalla. */
export function Notice({ kind = "info", children }: { kind?: NoticeKind; children: ReactNode }) {
  const className = kind === "info" ? "notice" : `notice notice-${kind}`;
  return (
    <div className={className} role={kind === "error" ? "alert" : undefined}>
      <div>{children}</div>
    </div>
  );
}

/** Cabecera de pantalla: título, subtítulo y acciones a la derecha. */
export function PageHeader({
  title,
  subtitle,
  actions,
}: {
  title: ReactNode;
  subtitle?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <div className="page-header">
      <div className="page-header-text">
        <h1>{title}</h1>
        {subtitle !== undefined && <div className="meta">{subtitle}</div>}
      </div>
      {actions}
    </div>
  );
}

/** Cifra destacada con su etiqueta. */
export function Stat({
  label,
  value,
  tone,
  detail,
  hint,
}: {
  label: string;
  value: ReactNode;
  tone?: "error";
  /** Línea secundaria bajo el valor. */
  detail?: ReactNode;
  /** Explicación al pasar el ratón. */
  hint?: string;
}) {
  return (
    <div className={tone === "error" ? "stat stat-error" : "stat"} title={hint}>
      <span className="stat-label">{label}</span>
      <span className="stat-value">{value}</span>
      {detail !== undefined && <span className="stat-detail">{detail}</span>}
    </div>
  );
}

/** Pantalla vacía con icono, texto y una acción. */
export function EmptyState({
  icon,
  title,
  children,
}: {
  icon: ReactNode;
  title: string;
  children?: ReactNode;
}) {
  return (
    <div className="empty">
      <span className="empty-icon">{icon}</span>
      <h3>{title}</h3>
      {children}
    </div>
  );
}
