export interface MenuItem {
  label: string;
  value?: string;
}

interface MenuProps {
  open: boolean;
  items: MenuItem[];
  index: number;
  onPoint: (i: number) => void;
  onPick: (i: number) => void;
  onClose: () => void;
}

export function Menu({ open, items, index, onPoint, onPick, onClose }: MenuProps) {
  return (
    <>
      <div className="scrim scrim-strong" data-open={open || undefined} onClick={onClose} />
      <section className="menu" data-open={open || undefined} aria-hidden={!open} role="menu">
        {items.map((item, i) => (
          <div
            key={item.label}
            role="menuitem"
            className="menu-item"
            data-focused={(open && i === index) || undefined}
            onPointerMove={() => onPoint(i)}
            onClick={() => onPick(i)}
          >
            <span>{item.label}</span>
            {item.value && <span className="menu-value">{item.value}</span>}
          </div>
        ))}
      </section>
    </>
  );
}
