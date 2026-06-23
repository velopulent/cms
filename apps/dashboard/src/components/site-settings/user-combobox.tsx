import { Check, ChevronsUpDown } from "lucide-react";
import type { ReactNode } from "react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@/components/ui/command";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import type { UserPublic } from "@/lib/api";
import { cn } from "@/lib/utils";

/** Searchable picker over a list of users, selecting one by id. */
export function UserCombobox({
  users,
  value,
  onChange,
  placeholder = "Select a user…",
  emptyText = "No users available.",
  footer,
}: {
  users: UserPublic[];
  value: string | null;
  onChange: (userId: string) => void;
  placeholder?: string;
  emptyText?: string;
  footer?: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const selected = users.find((user) => user.id === value);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        render={
          <Button
            variant="outline"
            role="combobox"
            aria-expanded={open}
            className="w-full justify-between font-normal"
          >
            <span
              className={cn("truncate", !selected && "text-muted-foreground")}
            >
              {selected ? selected.name : placeholder}
            </span>
            <ChevronsUpDown className="size-4 shrink-0 opacity-50" />
          </Button>
        }
      />
      <PopoverContent
        align="start"
        className="w-[var(--anchor-width)] min-w-60 p-0"
      >
        <Command>
          <CommandInput placeholder="Search users…" />
          <CommandList>
            <CommandEmpty>{emptyText}</CommandEmpty>
            {users.length > 0 && (
              <CommandGroup>
                {users.map((user) => (
                  <CommandItem
                    key={user.id}
                    value={`${user.name} ${user.email}`}
                    onSelect={() => {
                      onChange(user.id);
                      setOpen(false);
                    }}
                  >
                    <Check
                      className={cn(
                        "size-4",
                        value === user.id ? "opacity-100" : "opacity-0",
                      )}
                    />
                    <div className="flex min-w-0 flex-col">
                      <span className="truncate font-medium">{user.name}</span>
                      <span className="truncate text-xs text-muted-foreground">
                        {user.email}
                      </span>
                    </div>
                  </CommandItem>
                ))}
              </CommandGroup>
            )}
            {footer && (
              <>
                <CommandSeparator />
                <div className="p-1">{footer}</div>
              </>
            )}
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}
