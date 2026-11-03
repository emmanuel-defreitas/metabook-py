"use client";

import {
  Select,
  SelectItem,
  SelectPopup,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  DATE_OPTIONS,
  KIND_OPTIONS,
  type Option,
  SOURCE_OPTIONS,
  type SearchFilters,
} from "@/lib/filters";
import { cn } from "@/lib/utils";

function summary(values: string[], options: Option[], empty: string) {
  if (!values.length) return empty;
  const first = options.find((option) => option.value === values[0])?.label ?? values[0];
  return values.length > 1 ? `${first} +${values.length - 1}` : first;
}

function MultiFilter({
  label,
  empty,
  options,
  value,
  onChange,
  disabled,
}: {
  label: string;
  empty: string;
  options: Option[];
  value: string[];
  onChange: (value: string[]) => void;
  disabled?: boolean;
}) {
  return (
    <Select
      disabled={disabled}
      multiple
      onValueChange={(next) => onChange((next as string[]) ?? [])}
      value={value}
    >
      <SelectTrigger
        aria-label={label}
        className={cn("w-auto min-w-36 rounded-full", value.length && "border-primary/40")}
      >
        <SelectValue>{(selected: string[]) => summary(selected, options, empty)}</SelectValue>
      </SelectTrigger>
      <SelectPopup alignItemWithTrigger={false}>
        {options.map((option) => (
          <SelectItem disabled={option.disabled} key={option.value} value={option.value}>
            <span className="flex flex-col">
              <span>{option.label}</span>
              {option.note && (
                <span className="text-subtle-foreground text-xs">{option.note}</span>
              )}
            </span>
          </SelectItem>
        ))}
      </SelectPopup>
    </Select>
  );
}

/** Label-free Date / Category / Type multi-selects plus one Source select. */
export function FilterBar({
  filters,
  categories,
  onChange,
  disabled,
  className,
}: {
  filters: SearchFilters;
  categories: Option[];
  onChange: (filters: SearchFilters) => void;
  disabled?: boolean;
  className?: string;
}) {
  const sourceItems = Object.fromEntries(SOURCE_OPTIONS.map((o) => [o.value, o.label]));
  return (
    <div className={cn("flex flex-wrap items-center gap-2", className)}>
      <MultiFilter
        disabled={disabled}
        empty="All dates"
        label="Date"
        onChange={(date) => onChange({ ...filters, date })}
        options={DATE_OPTIONS}
        value={filters.date}
      />
      <MultiFilter
        disabled={disabled}
        empty="All categories"
        label="Category"
        onChange={(category) => onChange({ ...filters, category })}
        options={categories}
        value={filters.category}
      />
      <MultiFilter
        disabled={disabled}
        empty="All types"
        label="Type"
        onChange={(kind) => onChange({ ...filters, kind })}
        options={KIND_OPTIONS}
        value={filters.kind}
      />
      <Select
        disabled={disabled}
        items={sourceItems}
        onValueChange={(source) => onChange({ ...filters, source: (source as string) ?? "gutendex" })}
        value={filters.source}
      >
        <SelectTrigger aria-label="Source" className="w-auto min-w-32 rounded-full">
          <SelectValue />
        </SelectTrigger>
        <SelectPopup alignItemWithTrigger={false}>
          {SOURCE_OPTIONS.map((option) => (
            <SelectItem disabled={option.disabled} key={option.value} value={option.value}>
              <span className="flex flex-col">
                <span>{option.label}</span>
                {option.note && (
                  <span className="text-subtle-foreground text-xs">{option.note}</span>
                )}
              </span>
            </SelectItem>
          ))}
        </SelectPopup>
      </Select>
    </div>
  );
}
