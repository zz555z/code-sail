import { type RefObject, useCallback, useEffect, useRef } from "react";
import { ChevronDown } from "lucide-react";

type ModelComboboxProps = {
  value: string;
  models: string[];
  menuOpen: boolean;
  menuId?: string;
  ariaLabel?: string;
  placeholder?: string;
  disabled?: boolean;
  containerRef?: RefObject<HTMLDivElement>;
  onChange: (value: string) => void;
  onSelect: (model: string) => void;
  onMenuToggle: (open: boolean) => void;
  onFocus?: () => void;
};

export function ModelCombobox({
  value,
  models,
  menuOpen,
  menuId = "model-options",
  ariaLabel = "模型列表",
  placeholder = "选择模型或手动填写",
  disabled = false,
  containerRef,
  onChange,
  onSelect,
  onMenuToggle,
  onFocus
}: ModelComboboxProps) {
  const defaultRef = useRef<HTMLDivElement>(null);
  const ref = containerRef ?? defaultRef;
  const menuRef = useRef<HTMLDivElement>(null);

  const handleToggle = useCallback(() => {
    if (!models.length) return;
    onMenuToggle(!menuOpen);
  }, [models.length, menuOpen, onMenuToggle]);

  const handleSelect = useCallback(
    (model: string) => {
      onSelect(model);
      onMenuToggle(false);
    },
    [onSelect, onMenuToggle]
  );

  // 点击外部关闭菜单
  useEffect(() => {
    if (!menuOpen) return;

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Element)) return;
      if (ref.current?.contains(target) || menuRef.current?.contains(target)) return;
      onMenuToggle(false);
    };

    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, [menuOpen, onMenuToggle, ref]);

  return (
    <div className="model-combobox">
      <div
        className="model-input-wrap"
        ref={ref}
        onBlur={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget)) {
            onMenuToggle(false);
          }
        }}
      >
        <input
          value={value}
          role="combobox"
          aria-expanded={menuOpen}
          aria-controls={menuId}
          aria-autocomplete="list"
          aria-label={ariaLabel}
          onFocus={() => {
            onMenuToggle(models.length > 0);
            onFocus?.();
          }}
          onClick={() => onMenuToggle(models.length > 0)}
          onChange={(event) => {
            onChange(event.target.value);
            onMenuToggle(models.length > 0);
          }}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              onMenuToggle(false);
            }
          }}
          placeholder={placeholder}
          disabled={disabled}
        />
        <button
          className={`model-menu-toggle ${menuOpen ? "open" : ""}`}
          type="button"
          data-tooltip="展开模型列表"
          data-tooltip-placement="left"
          aria-label="展开模型列表"
          disabled={!models.length || disabled}
          onMouseDown={(event) => event.preventDefault()}
          onClick={handleToggle}
        >
          <ChevronDown size={17} />
        </button>
        {menuOpen && models.length ? (
          <div
            ref={menuRef}
            className="model-menu"
            id={menuId}
            role="listbox"
            aria-label={ariaLabel}
          >
            {models.map((model) => (
              <button
                key={model}
                className={`model-option ${model === value ? "selected" : ""}`}
                type="button"
                role="option"
                aria-selected={model === value}
                onMouseDown={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  handleSelect(model);
                }}
              >
                {model}
              </button>
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
}
