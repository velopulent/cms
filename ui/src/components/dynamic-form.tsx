import { TiptapEditor } from "@/components/tiptap-editor";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import type { ContentField } from "@/lib/api";

interface DynamicFormProps {
  fields: ContentField[];
  values: Record<string, unknown>;
  onChange: (values: Record<string, unknown>) => void;
}

export function DynamicForm({ fields, values, onChange }: DynamicFormProps) {
  const updateField = (name: string, value: unknown) => {
    onChange({ ...values, [name]: value });
  };

  return (
    <div className="flex flex-col gap-4">
      {fields.map((field) => (
        <DynamicField
          key={field.name}
          field={field}
          value={values[field.name]}
          onChange={(val) => updateField(field.name, val)}
        />
      ))}
    </div>
  );
}

function DynamicField({
  field,
  value,
  onChange,
}: {
  field: ContentField;
  value: unknown;
  onChange: (val: unknown) => void;
}) {
  const label = field.name
    .replace(/_/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());

  return (
    <div className="flex flex-col gap-2">
      <p className="text-sm font-medium">
        {label}
        {field.required && <span className="ml-1 text-destructive">*</span>}
      </p>
      <FieldInput field={field} value={value} onChange={onChange} />
    </div>
  );
}

function FieldInput({
  field,
  value,
  onChange,
}: {
  field: ContentField;
  value: unknown;
  onChange: (val: unknown) => void;
}) {
  const strValue = typeof value === "string" ? value : "";
  const numValue = typeof value === "number" ? value : 0;
  const boolValue = typeof value === "boolean" ? value : false;

  switch (field.type) {
    case "text":
      return (
        <Input
          placeholder={field.name}
          value={strValue}
          onChange={(e) => onChange(e.target.value)}
        />
      );

    case "textarea":
      return (
        <Textarea
          placeholder={field.name}
          value={strValue}
          onChange={(e) => onChange(e.target.value)}
          rows={4}
        />
      );

    case "rich_text":
      return (
        <TiptapEditor
          content={strValue}
          onChange={(html) => onChange(html)}
          placeholder={`Write ${field.name}...`}
        />
      );

    case "number":
      return (
        <Input
          type="number"
          value={numValue}
          onChange={(e) => onChange(Number(e.target.value) || 0)}
        />
      );

    case "boolean":
      return (
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={boolValue}
            onChange={(e) => onChange(e.target.checked)}
          />
          Enabled
        </label>
      );

    case "date":
      return (
        <Input
          type="date"
          value={strValue}
          onChange={(e) => onChange(e.target.value)}
        />
      );

    case "select":
      return (
        <Select
          items={[
            { label: `Select ${field.name}`, value: null },
            ...(field.options ?? []).map((opt) => ({
              label: opt,
              value: opt,
            })),
          ]}
          value={strValue}
          onValueChange={(val) => onChange(val as string)}
        >
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              {(field.options ?? []).map((opt) => (
                <SelectItem key={opt} value={opt}>
                  {opt}
                </SelectItem>
              ))}
            </SelectGroup>
          </SelectContent>
        </Select>
      );

    case "image_url":
      return (
        <div className="flex flex-col gap-2">
          <Input
            placeholder="Image URL"
            value={strValue}
            onChange={(e) => onChange(e.target.value)}
          />
          {strValue && (
            <img
              src={strValue}
              alt="Preview"
              className="h-32 w-auto rounded-lg border object-cover"
            />
          )}
        </div>
      );

    default:
      return (
        <Input value={strValue} onChange={(e) => onChange(e.target.value)} />
      );
  }
}
