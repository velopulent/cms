import type { DragEndEvent } from "@dnd-kit/core";
import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import {
  AlignLeft,
  Archive as ArchiveIcon,
  Braces,
  Calendar,
  Copy,
  FileText,
  GripVertical,
  Hash,
  Image as ImageIcon,
  Layers,
  Link as LinkIcon,
  List,
  Mail,
  MoreHorizontal,
  Music,
  Pencil,
  Plus,
  Settings2,
  Share2,
  ToggleLeft,
  Trash2,
  Type as TypeIcon,
  Video,
} from "lucide-react";
import { type ComponentType, type ReactNode, useState } from "react";
import { toast } from "sonner";
import { z } from "zod";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Field,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Sheet,
  SheetClose,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  type Collection,
  type ContentField,
  createCollection,
  deleteCollection,
  getCollections,
  type SchemaDefinition,
  updateCollection,
} from "@/lib/api";

interface ContentFieldWithId extends ContentField {
  _id: string;
}

export const Route = createFileRoute("/_admin/sites/$siteId/collections")({
  component: CollectionsPage,
});

type FieldTypeMeta = {
  value: string;
  label: string;
  icon: ComponentType<{ className?: string }>;
};

const FIELD_TYPES: FieldTypeMeta[] = [
  { value: "text", label: "Plain text", icon: TypeIcon },
  { value: "textarea", label: "Text Area", icon: AlignLeft },
  { value: "rich_text", label: "Rich editor", icon: Pencil },
  { value: "number", label: "Number", icon: Hash },
  { value: "boolean", label: "Bool", icon: ToggleLeft },
  { value: "email", label: "Email", icon: Mail },
  { value: "url", label: "URL", icon: LinkIcon },
  { value: "date", label: "Datetime", icon: Calendar },
  { value: "select", label: "Select", icon: List },
  { value: "json", label: "JSON", icon: Braces },
  { value: "relation", label: "Relation", icon: Share2 },
  { value: "image_url", label: "Image URL", icon: LinkIcon },
  { value: "image", label: "Image", icon: ImageIcon },
  { value: "video", label: "Video", icon: Video },
  { value: "audio", label: "Audio", icon: Music },
  { value: "document", label: "Document", icon: FileText },
  { value: "archive", label: "Archive", icon: ArchiveIcon },
];

const FIELD_TYPE_MAP: Record<string, FieldTypeMeta> = Object.fromEntries(
  FIELD_TYPES.map((ft) => [ft.value, ft]),
);

function fieldTypeMeta(type: string): FieldTypeMeta {
  return FIELD_TYPE_MAP[type] ?? FIELD_TYPES[0];
}

/** Types whose value can be single or multiple (array). */
const MULTI_VALUE_TYPES = [
  "select",
  "relation",
  "image",
  "video",
  "audio",
  "document",
  "archive",
];

const CONTENT_TYPE_MIME_TYPES: Record<string, string[]> = {
  image: [
    "image/jpeg",
    "image/png",
    "image/gif",
    "image/webp",
    "image/avif",
    "image/svg+xml",
  ],
  video: ["video/mp4", "video/webm", "video/ogg", "video/quicktime"],
  audio: ["audio/mpeg", "audio/wav", "audio/ogg", "audio/webm", "audio/aac"],
  document: [
    "application/pdf",
    "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.ms-excel",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "text/plain",
    "text/csv",
    "text/markdown",
  ],
  archive: [
    "application/zip",
    "application/gzip",
    "application/x-tar",
    "application/x-7z-compressed",
  ],
};

function slugify(text: string) {
  return text
    .toLowerCase()
    .trim()
    .replace(/[^\w\s-]/g, "")
    .replace(/[\s_]+/g, "-")
    .replace(/-+/g, "-");
}

function CollectionsPage() {
  const { siteId } = Route.useParams();
  const queryClient = useQueryClient();
  const [createOpen, setCreateOpen] = useState(false);
  const [editCollection, setEditCollection] = useState<Collection | null>(null);
  const [pendingIsSingleton, setPendingIsSingleton] = useState(false);

  const { data: collections, isLoading } = useQuery({
    queryKey: ["collections", siteId],
    queryFn: () => getCollections(siteId),
  });

  const deleteMutation = useMutation({
    mutationFn: (slug: string) => deleteCollection(siteId, slug),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["collections", siteId] });
      toast.success("Collection deleted");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  return (
    <div className="flex flex-col gap-6 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold">Collections</h1>
          <p className="text-sm text-muted-foreground">
            Define the structure of your content
          </p>
        </div>
        <Sheet
          open={createOpen}
          onOpenChange={(open) => {
            setCreateOpen(open);
            if (!open) setPendingIsSingleton(false);
          }}
        >
          <SheetTrigger render={<Button />}>
            <Plus data-icon="inline-start" />
            New
          </SheetTrigger>
          <SheetContent
            className={
              "data-[side=right]:w-full data-[side=right]:sm:max-w-2xl"
            }
          >
            <SheetHeader>
              <SheetTitle>
                {pendingIsSingleton ? "Create Singleton" : "Create Collection"}
              </SheetTitle>
              <SheetDescription>
                {pendingIsSingleton
                  ? "Define a new singleton with custom fields."
                  : "Define a new collection with custom fields."}
              </SheetDescription>
            </SheetHeader>
            <div className="flex-1 overflow-y-auto px-4">
              <CollectionForm
                siteId={siteId}
                onIsSingletonChange={setPendingIsSingleton}
                onSubmit={(data) => {
                  createCollection(siteId, data)
                    .then(() => {
                      queryClient.invalidateQueries({
                        queryKey: ["collections", siteId],
                      });
                      setCreateOpen(false);
                      setPendingIsSingleton(false);
                      toast.success(
                        data.is_singleton
                          ? "Singleton created"
                          : "Collection created",
                      );
                    })
                    .catch((err: Error) => toast.error(err.message));
                }}
              />
            </div>
            <SheetFooter>
              <Button
                type="submit"
                form="collection-form-create"
                disabled={false}
              >
                {pendingIsSingleton ? "Create Singleton" : "Create Collection"}
              </Button>
              <SheetClose
                render={
                  <Button type="button" variant="outline">
                    Cancel
                  </Button>
                }
              />
            </SheetFooter>
          </SheetContent>
        </Sheet>
      </div>

      {isLoading ? (
        <div className="flex flex-col gap-2">
          <Skeleton className="h-12 w-full" />
          <Skeleton className="h-12 w-full" />
        </div>
      ) : !collections?.length ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Layers className="mb-4 size-10 text-muted-foreground" />
            <p className="text-lg font-medium">No collections yet</p>
            <p className="text-sm text-muted-foreground">
              Create your first collection or singleton to get started.
            </p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Type</TableHead>
                <TableHead>Slug</TableHead>
                <TableHead>Fields</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {collections.map((c: Collection) => {
                let fieldCount = 0;
                try {
                  const def: SchemaDefinition = JSON.parse(c.definition);
                  fieldCount = def.fields?.length ?? 0;
                } catch {
                  // invalid json
                }
                return (
                  <TableRow key={c.id}>
                    <TableCell className="font-medium">{c.name}</TableCell>
                    <TableCell>
                      <Badge variant={c.is_singleton ? "secondary" : "outline"}>
                        {c.is_singleton ? "Singleton" : "Collection"}
                      </Badge>
                    </TableCell>
                    <TableCell>
                      <Badge variant="outline">{c.slug}</Badge>
                    </TableCell>
                    <TableCell>{fieldCount} fields</TableCell>
                    <TableCell className="text-right">
                      <div className="flex justify-end gap-1">
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => setEditCollection(c)}
                        >
                          <Pencil />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => deleteMutation.mutate(c.slug)}
                          disabled={deleteMutation.isPending}
                        >
                          <Trash2 />
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </Card>
      )}

      <Sheet
        open={editCollection !== null}
        onOpenChange={(open) => !open && setEditCollection(null)}
      >
        <SheetContent
          className={"data-[side=right]:w-full data-[side=right]:sm:max-w-xl"}
        >
          <SheetHeader>
            <SheetTitle>
              {editCollection?.is_singleton
                ? "Edit Singleton"
                : "Edit Collection"}
            </SheetTitle>
            <SheetDescription>
              {editCollection?.is_singleton
                ? "Update the singleton definition."
                : "Update the collection definition."}
            </SheetDescription>
          </SheetHeader>
          <div className="flex-1 overflow-y-auto px-4">
            {editCollection && (
              <CollectionForm
                key={editCollection.id}
                siteId={siteId}
                initialData={editCollection}
                onSubmit={(data) => {
                  updateCollection(siteId, editCollection.slug, {
                    name: data.name,
                    slug: data.slug,
                    definition: data.definition,
                  })
                    .then(() => {
                      queryClient.invalidateQueries({
                        queryKey: ["collections", siteId],
                      });
                      const wasSingleton = editCollection.is_singleton;
                      setEditCollection(null);
                      toast.success(
                        wasSingleton
                          ? "Singleton updated"
                          : "Collection updated",
                      );
                    })
                    .catch((err: Error) => toast.error(err.message));
                }}
              />
            )}
          </div>
          <SheetFooter>
            <Button type="submit" form="collection-form-edit" disabled={false}>
              {editCollection?.is_singleton
                ? "Update Singleton"
                : "Update Collection"}
            </Button>
            <SheetClose
              render={
                <Button type="button" variant="outline">
                  Cancel
                </Button>
              }
            />
          </SheetFooter>
        </SheetContent>
      </Sheet>
    </div>
  );
}

// --- Sortable Field Item ---

// biome-ignore lint/suspicious/noExplicitAny: TanStack Form instance with complex generics
type FieldForm = any;

interface FieldConfigProps {
  field: ContentFieldWithId;
  index: number;
  form: FieldForm;
  set: (key: keyof ContentField, value: unknown) => void;
  siteId: string;
}

function SortableFieldItem({
  field,
  index,
  form,
  removeField,
  duplicateField,
  siteId,
}: {
  field: ContentFieldWithId;
  index: number;
  form: FieldForm;
  removeField: (index: number) => void;
  duplicateField: (index: number) => void;
  siteId: string;
}) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: field._id });

  const [open, setOpen] = useState(!field.name);
  const [nameTouched, setNameTouched] = useState(false);
  const nameInvalid = nameTouched && !field.name.trim();
  const meta = fieldTypeMeta(field.type);
  const Icon = meta.icon;
  const supportsMultiple = MULTI_VALUE_TYPES.includes(field.type);

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
  };

  const set = (key: keyof ContentField, value: unknown) =>
    form.setFieldValue(`fields[${index}].${key}`, value);

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <div ref={setNodeRef} style={style} className="rounded-lg border bg-card">
        <div className="flex items-center gap-1.5 p-2">
          <button
            type="button"
            className="cursor-grab text-muted-foreground hover:text-foreground"
            aria-label="Drag to reorder"
            {...attributes}
            {...listeners}
          >
            <GripVertical className="size-4" />
          </button>
          <Icon className="size-4 shrink-0 text-muted-foreground" />
          <Field data-invalid={nameInvalid} className="min-w-0 flex-1">
            <Input
              placeholder="fieldName"
              value={field.name}
              onBlur={() => setNameTouched(true)}
              onChange={(e) => set("name", e.target.value)}
              aria-invalid={nameInvalid}
              className="h-8 border-transparent bg-transparent px-2 shadow-none focus-visible:border-input focus-visible:bg-background"
            />
          </Field>
          {supportsMultiple && (
            <Select
              items={[
                { label: "Single", value: "single" },
                { label: "Multiple", value: "multiple" },
              ]}
              value={field.multiple ? "multiple" : "single"}
              onValueChange={(val) => set("multiple", val === "multiple")}
            >
              <SelectTrigger className="hidden h-8 w-[108px] sm:flex">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectItem value="single">Single</SelectItem>
                  <SelectItem value="multiple">Multiple</SelectItem>
                </SelectGroup>
              </SelectContent>
            </Select>
          )}
          {field.required && (
            <Badge
              variant="secondary"
              className="hidden shrink-0 sm:inline-flex"
            >
              Required
            </Badge>
          )}
          <CollapsibleTrigger
            render={
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                aria-label="Field settings"
              />
            }
          >
            <Settings2 />
          </CollapsibleTrigger>
        </div>
        {nameInvalid && (
          <p className="px-3 pb-2 text-xs text-destructive">
            Field name is required
          </p>
        )}
        <CollapsibleContent>
          <div className="flex flex-col gap-4 border-t p-3 sm:p-4">
            <Field className="max-w-xs">
              <FieldLabel className="text-xs">Type</FieldLabel>
              <Select
                items={FIELD_TYPES.map((ft) => ({
                  label: ft.label,
                  value: ft.value,
                }))}
                value={field.type}
                onValueChange={(val) => set("type", val as string)}
              >
                <SelectTrigger className="h-8 w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    {FIELD_TYPES.map((ft) => (
                      <SelectItem key={ft.value} value={ft.value}>
                        {ft.label}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>

            <TypeConfig
              field={field}
              index={index}
              form={form}
              set={set}
              siteId={siteId}
            />

            <ConfigBox label="Help text">
              <Input
                placeholder="Shown under the field in the editor"
                value={field.help ?? ""}
                onChange={(e) => set("help", e.target.value || undefined)}
                className="h-8 border-0 bg-transparent px-0 shadow-none focus-visible:ring-0"
              />
            </ConfigBox>

            <div className="flex flex-wrap items-center gap-4">
              <FlagCheckbox
                id={`required-${field._id}`}
                label="Required"
                checked={field.required ?? false}
                onChange={(c) => set("required", c)}
              />
              <FlagCheckbox
                id={`presentable-${field._id}`}
                label="Presentable"
                checked={field.presentable ?? false}
                onChange={(c) => set("presentable", c)}
              />
              <FlagCheckbox
                id={`hidden-${field._id}`}
                label="Hidden"
                checked={field.hidden ?? false}
                onChange={(c) => set("hidden", c)}
              />
              <DropdownMenu>
                <DropdownMenuTrigger
                  render={
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      className="ml-auto"
                      aria-label="More options"
                    />
                  }
                >
                  <MoreHorizontal />
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuItem onClick={() => duplicateField(index)}>
                    <Copy />
                    Duplicate
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    variant="destructive"
                    onClick={() => removeField(index)}
                  >
                    <Trash2 />
                    Delete
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
        </CollapsibleContent>
      </div>
    </Collapsible>
  );
}

// --- Field config building blocks ---

function ConfigBox({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1 rounded-md border bg-muted/30 p-3">
      <span className="text-xs font-medium text-muted-foreground">{label}</span>
      {children}
      {hint && <span className="text-xs text-muted-foreground">{hint}</span>}
    </div>
  );
}

function NumberConfig({
  label,
  hint,
  placeholder,
  value,
  onChange,
}: {
  label: string;
  hint?: string;
  placeholder?: string;
  value: number | undefined;
  onChange: (v: number | undefined) => void;
}) {
  return (
    <ConfigBox label={label} hint={hint}>
      <Input
        type="number"
        placeholder={placeholder}
        value={value ?? ""}
        onChange={(e) =>
          onChange(e.target.value === "" ? undefined : Number(e.target.value))
        }
        className="h-8 border-0 bg-transparent px-0 shadow-none focus-visible:ring-0"
      />
    </ConfigBox>
  );
}

function TextConfig({
  label,
  hint,
  placeholder,
  value,
  onChange,
}: {
  label: string;
  hint?: string;
  placeholder?: string;
  value: string | undefined;
  onChange: (v: string | undefined) => void;
}) {
  return (
    <ConfigBox label={label} hint={hint}>
      <Input
        placeholder={placeholder}
        value={value ?? ""}
        onChange={(e) => onChange(e.target.value || undefined)}
        className="h-8 border-0 bg-transparent px-0 font-mono text-xs shadow-none focus-visible:ring-0"
      />
    </ConfigBox>
  );
}

function FlagCheckbox({
  id,
  label,
  checked,
  onChange,
}: {
  id: string;
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <Field orientation="horizontal" className="w-auto">
      <Checkbox
        id={id}
        checked={checked}
        onCheckedChange={(c) => onChange(!!c)}
      />
      <FieldLabel htmlFor={id} className="font-normal text-sm">
        {label}
      </FieldLabel>
    </Field>
  );
}

function TypeConfig({ field, set, siteId }: FieldConfigProps) {
  switch (field.type) {
    case "text":
      return (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <NumberConfig
            label="Min length"
            placeholder="No min limit"
            value={field.min_length}
            onChange={(v) => set("min_length", v)}
          />
          <NumberConfig
            label="Max length"
            placeholder="Default to max 5000"
            value={field.max_length}
            onChange={(v) => set("max_length", v)}
          />
          <div className="sm:col-span-2">
            <TextConfig
              label="Validation pattern"
              hint="Ex. ^[a-z0-9]+$"
              placeholder="^[a-z0-9]+$"
              value={field.pattern}
              onChange={(v) => set("pattern", v)}
            />
          </div>
        </div>
      );
    case "textarea":
      return (
        <NumberConfig
          label="Max length"
          placeholder="Default to max 5000 characters"
          value={field.max_length}
          onChange={(v) => set("max_length", v)}
        />
      );
    case "rich_text":
      return (
        <NumberConfig
          label="Max size (bytes)"
          placeholder="Default to max ~5MB"
          value={field.max_size}
          onChange={(v) => set("max_size", v)}
        />
      );
    case "number":
      return (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <NumberConfig
            label="Min"
            value={field.min}
            onChange={(v) => set("min", v)}
          />
          <NumberConfig
            label="Max"
            value={field.max}
            onChange={(v) => set("max", v)}
          />
        </div>
      );
    case "json":
      return (
        <NumberConfig
          label="Max size (bytes)"
          placeholder="Default to max ~5MB"
          value={field.max_size}
          onChange={(v) => set("max_size", v)}
        />
      );
    case "select":
      return <SelectOptionsConfig field={field} set={set} />;
    case "relation":
      return <RelationConfig field={field} set={set} siteId={siteId} />;
    case "image":
    case "video":
    case "audio":
    case "document":
    case "archive":
      return <FileConfig field={field} set={set} />;
    default:
      return null;
  }
}

function SelectOptionsConfig({
  field,
  set,
}: {
  field: ContentFieldWithId;
  set: (key: keyof ContentField, value: unknown) => void;
}) {
  return (
    <ConfigBox label="Options">
      <div className="flex flex-wrap gap-1">
        {(field.options ?? []).map((opt, optIdx) => (
          <span
            key={opt}
            className="inline-flex items-center gap-1 rounded-md border border-border bg-background px-2 py-0.5 text-xs"
          >
            {opt}
            <button
              type="button"
              className="ml-0.5 text-muted-foreground hover:text-foreground"
              onClick={() =>
                set(
                  "options",
                  (field.options ?? []).filter((_, i) => i !== optIdx),
                )
              }
            >
              ×
            </button>
          </span>
        ))}
      </div>
      <Input
        placeholder="Add option and press Enter"
        className="mt-1 h-8 text-xs"
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            const input = e.target as HTMLInputElement;
            const val = input.value.trim();
            if (val) {
              set("options", [...(field.options ?? []), val]);
              input.value = "";
            }
          }
        }}
      />
    </ConfigBox>
  );
}

function FileConfig({
  field,
  set,
}: {
  field: ContentFieldWithId;
  set: (key: keyof ContentField, value: unknown) => void;
}) {
  const presets = CONTENT_TYPE_MIME_TYPES[field.type] ?? [];
  const acceptCount = (field.accept ?? []).length;
  return (
    <div className="flex flex-col gap-3">
      <ConfigBox
        label="Allowed mime types"
        hint={
          acceptCount === 0
            ? "No restriction — all files in this category accepted"
            : `${acceptCount} type${acceptCount === 1 ? "" : "s"} selected`
        }
      >
        <div className="flex flex-wrap gap-1">
          {presets.map((mime) => {
            const selected = (field.accept ?? []).includes(mime);
            return (
              <button
                key={mime}
                type="button"
                className={`inline-flex items-center rounded-md border px-2 py-0.5 text-xs transition-colors ${
                  selected
                    ? "border-primary bg-primary/10 text-primary"
                    : "border-border bg-background text-muted-foreground hover:text-foreground"
                }`}
                onClick={() => {
                  const current = field.accept ?? [];
                  const next = selected
                    ? current.filter((a) => a !== mime)
                    : [...current, mime];
                  set("accept", next.length > 0 ? next : undefined);
                }}
              >
                {mime.split("/")[1] ?? mime}
              </button>
            );
          })}
        </div>
      </ConfigBox>
      <NumberConfig
        label="Max size (bytes)"
        hint="In bytes. Default ~5MB."
        value={field.max_size}
        onChange={(v) => set("max_size", v)}
      />
      <FlagCheckbox
        id={`protected-${field._id}`}
        label="Protected"
        checked={field.protected ?? false}
        onChange={(c) => set("protected", c)}
      />
    </div>
  );
}

function RelationConfig({
  field,
  set,
  siteId,
}: {
  field: ContentFieldWithId;
  set: (key: keyof ContentField, value: unknown) => void;
  siteId: string;
}) {
  const { data: collections } = useQuery({
    queryKey: ["collections", siteId],
    queryFn: () => getCollections(siteId),
  });
  const items = (collections ?? []).map((c: Collection) => ({
    label: c.name,
    value: c.slug,
  }));
  return (
    <div className="flex flex-col gap-3">
      <ConfigBox label="Target collection">
        <Select
          items={items}
          value={field.target_collection ?? ""}
          onValueChange={(val) => set("target_collection", val as string)}
        >
          <SelectTrigger className="h-8 w-full border-0 bg-transparent px-0 shadow-none">
            <SelectValue placeholder="Select a collection" />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              {items.map((it) => (
                <SelectItem key={it.value} value={it.value}>
                  {it.label}
                </SelectItem>
              ))}
            </SelectGroup>
          </SelectContent>
        </Select>
      </ConfigBox>
      {field.multiple && (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <NumberConfig
            label="Min select"
            value={field.min_select}
            onChange={(v) => set("min_select", v)}
          />
          <NumberConfig
            label="Max select"
            value={field.max_select}
            onChange={(v) => set("max_select", v)}
          />
        </div>
      )}
      <FlagCheckbox
        id={`cascade-${field._id}`}
        label="Cascade delete"
        checked={field.cascade_delete ?? false}
        onChange={(c) => set("cascade_delete", c)}
      />
    </div>
  );
}

// --- Collection Form ---

const collectionFieldSchema = z.object({
  name: z.string().min(1, "Field name is required"),
  type: z.string(),
  required: z.boolean().optional(),
  options: z.array(z.string()).optional(),
  accept: z.array(z.string()).optional(),
  _id: z.string(),
});

const collectionFormSchema = z.object({
  name: z.string().min(1, "Collection name is required"),
  slug: z.string().min(1, "Slug is required"),
  fields: z.array(collectionFieldSchema).min(1, "Add at least one field"),
  is_singleton: z.boolean(),
});

type CollectionFormValues = {
  name: string;
  slug: string;
  fields: ContentFieldWithId[];
  is_singleton: boolean;
};

function CollectionForm({
  siteId,
  initialData,
  onSubmit,
  onIsSingletonChange,
}: {
  siteId: string;
  initialData?: Collection;
  onSubmit: (data: {
    name: string;
    slug: string;
    definition: SchemaDefinition;
    is_singleton?: boolean;
  }) => void;
  onIsSingletonChange?: (isSingleton: boolean) => void;
}) {
  const [slugManuallyEdited, setSlugManuallyEdited] = useState(!!initialData);
  const isEdit = !!initialData;

  const initialFields: ContentFieldWithId[] = (() => {
    if (initialData) {
      try {
        const def: SchemaDefinition = JSON.parse(initialData.definition);
        return (def.fields ?? []).map((f, i) => ({
          ...f,
          _id: `init-${i}`,
        }));
      } catch {
        return [];
      }
    }
    return [{ name: "", type: "text", required: false, _id: "default-0" }];
  })();

  const form = useForm({
    defaultValues: {
      name: initialData?.name ?? "",
      slug: initialData?.slug ?? "",
      fields: initialFields,
      is_singleton: initialData?.is_singleton ?? false,
    } as CollectionFormValues,
    validators: {
      onSubmit: collectionFormSchema,
    },
    onSubmit: async ({ value }) => {
      const cleanFields: ContentField[] = value.fields.map(
        ({ _id, ...rest }) => rest,
      );
      onSubmit({
        name: value.name,
        slug: value.slug,
        definition: { fields: cleanFields },
        is_singleton: value.is_singleton,
      });
    },
  });

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8,
      },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (over && active.id !== over.id) {
      const currentFields = form.getFieldValue("fields");
      const oldIndex = currentFields.findIndex((f) => f._id === active.id);
      const newIndex = currentFields.findIndex((f) => f._id === over.id);
      if (oldIndex !== -1 && newIndex !== -1) {
        form.setFieldValue(
          "fields",
          arrayMove(currentFields, oldIndex, newIndex),
        );
      }
    }
  };

  const addFieldOfType = (type: string) => {
    form.pushFieldValue("fields", {
      name: "",
      type,
      required: false,
      _id: `new-${Date.now()}-${form.getFieldValue("fields").length}`,
    });
  };

  const removeField = (index: number) => {
    form.removeFieldValue("fields", index);
  };

  const duplicateField = (index: number) => {
    const current = form.getFieldValue("fields") as ContentFieldWithId[];
    const original = current[index];
    if (!original) return;
    const copy: ContentFieldWithId = {
      ...original,
      name: original.name ? `${original.name}_copy` : "",
      _id: `dup-${Date.now()}-${current.length}`,
    };
    form.setFieldValue("fields", [
      ...current.slice(0, index + 1),
      copy,
      ...current.slice(index + 1),
    ]);
  };

  const formId = initialData
    ? `collection-form-edit`
    : `collection-form-create`;

  return (
    <form
      id={formId}
      onSubmit={(e) => {
        e.preventDefault();
        form.handleSubmit();
      }}
      className="flex flex-col gap-4 pb-4"
    >
      <FieldGroup>
        <form.Field
          name="name"
          children={(field) => {
            const isInvalid =
              field.state.meta.isTouched && !field.state.meta.isValid;
            return (
              <Field data-invalid={isInvalid}>
                <FieldLabel htmlFor={field.name}>Name</FieldLabel>
                <Input
                  id={field.name}
                  placeholder="e.g. Blog Post"
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(e) => {
                    field.handleChange(e.target.value);
                    if (!slugManuallyEdited) {
                      form.setFieldValue("slug", slugify(e.target.value));
                    }
                  }}
                  aria-invalid={isInvalid}
                />
                {isInvalid && <FieldError errors={field.state.meta.errors} />}
              </Field>
            );
          }}
        />
        <form.Field
          name="slug"
          children={(field) => {
            const isInvalid =
              field.state.meta.isTouched && !field.state.meta.isValid;
            return (
              <Field data-invalid={isInvalid}>
                <FieldLabel htmlFor={field.name}>Slug</FieldLabel>
                <Input
                  id={field.name}
                  placeholder="e.g. blog-post"
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(e) => {
                    field.handleChange(e.target.value);
                    setSlugManuallyEdited(true);
                  }}
                  aria-invalid={isInvalid}
                />
                {isInvalid && <FieldError errors={field.state.meta.errors} />}
              </Field>
            );
          }}
        />
      </FieldGroup>

      {!isEdit && (
        <form.Field
          name="is_singleton"
          children={(field) => (
            <Field>
              <FieldLabel>Type</FieldLabel>
              <Select
                items={[
                  { label: "Collection (multiple entries)", value: "false" },
                  { label: "Singleton (single entry)", value: "true" },
                ]}
                value={field.state.value ? "true" : "false"}
                onValueChange={(val) => {
                  const next = val === "true";
                  field.handleChange(next);
                  onIsSingletonChange?.(next);
                }}
              >
                <SelectTrigger className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectItem value="false">
                      Collection (multiple entries)
                    </SelectItem>
                    <SelectItem value="true">
                      Singleton (single entry)
                    </SelectItem>
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
          )}
        />
      )}

      <form.Field
        name="fields"
        children={(arrayField) => {
          const fields = arrayField.state.value;
          return (
            <div className="flex flex-col gap-3">
              <p className="text-sm font-medium">Fields</p>
              {fields.length === 0 && (
                <Field data-invalid={!arrayField.state.meta.isValid}>
                  <p className="text-sm text-muted-foreground">
                    No fields defined yet.
                  </p>
                  {!arrayField.state.meta.isValid && (
                    <FieldError errors={arrayField.state.meta.errors} />
                  )}
                </Field>
              )}
              <DndContext
                sensors={sensors}
                collisionDetection={closestCenter}
                onDragEnd={handleDragEnd}
              >
                <SortableContext
                  items={fields.map((f) => f._id)}
                  strategy={verticalListSortingStrategy}
                >
                  <div className="flex flex-col gap-2">
                    {fields.map((field, index) => (
                      <SortableFieldItem
                        key={field._id}
                        field={field}
                        index={index}
                        form={form}
                        removeField={removeField}
                        duplicateField={duplicateField}
                        siteId={siteId}
                      />
                    ))}
                  </div>
                </SortableContext>
              </DndContext>
              <NewFieldPopover onPick={addFieldOfType} />
            </div>
          );
        }}
      />
    </form>
  );
}

// --- New field type picker (Popover grid) ---

function NewFieldPopover({ onPick }: { onPick: (type: string) => void }) {
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        render={
          <Button
            type="button"
            variant="outline"
            className="w-full border-dashed"
          />
        }
      >
        <Plus data-icon="inline-start" />
        New field
      </PopoverTrigger>
      <PopoverContent
        align="center"
        className="min-w-[min(28rem,calc(100vw-2rem))] w-full"
      >
        <div className="grid grid-cols-2 gap-1 sm:grid-cols-4">
          {FIELD_TYPES.map((ft) => {
            const Icon = ft.icon;
            return (
              <button
                key={ft.value}
                type="button"
                className="flex items-center gap-2 rounded-md px-2 py-2 text-left text-sm hover:bg-accent"
                onClick={() => {
                  onPick(ft.value);
                  setOpen(false);
                }}
              >
                <Icon className="size-4 shrink-0 text-muted-foreground" />
                <span className="truncate">{ft.label}</span>
              </button>
            );
          })}
        </div>
      </PopoverContent>
    </Popover>
  );
}
