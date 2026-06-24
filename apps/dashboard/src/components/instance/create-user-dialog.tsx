import { useMutation, useQueryClient } from "@tanstack/react-query";
import { ShieldCheck } from "lucide-react";
import { type ReactElement, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  createManagedUser,
  INSTANCE_ROLE_ITEMS,
  ROLE_USER,
  type RoleValue,
  type UserPublic,
} from "@/lib/api";

const EMPTY = {
  name: "",
  email: "",
  temporary_password: "",
  role: ROLE_USER as RoleValue,
};

/**
 * Reusable dialog for creating a managed user. Used from instance settings and
 * from a site's members page. Only owners may grant the owner role.
 */
export function CreateUserDialog({
  currentUserIsOwner,
  onCreated,
  trigger,
}: {
  currentUserIsOwner: boolean;
  onCreated?: (user: UserPublic) => void;
  trigger: ReactElement;
}) {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const [form, setForm] = useState(EMPTY);

  const createMutation = useMutation({
    mutationFn: () =>
      createManagedUser({
        name: form.name,
        email: form.email,
        temporary_password: form.temporary_password,
        instance_role: form.role === ROLE_USER ? null : form.role,
      }),
    onSuccess: (user) => {
      queryClient.invalidateQueries({ queryKey: ["instance-users"] });
      setOpen(false);
      setForm(EMPTY);
      onCreated?.(user);
      toast.success("User created");
    },
    onError: (error: Error) => toast.error(error.message),
  });

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger render={trigger} />
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Create user</DialogTitle>
          <DialogDescription>
            The user must change their temporary password after signing in.
          </DialogDescription>
        </DialogHeader>
        <form
          className="flex flex-col gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            createMutation.mutate();
          }}
        >
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="managed-name">Name</FieldLabel>
              <Input
                id="managed-name"
                required
                minLength={1}
                placeholder="John Doe"
                value={form.name}
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    name: event.target.value,
                  }))
                }
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="managed-email">Email</FieldLabel>
              <Input
                id="managed-email"
                type="email"
                required
                value={form.email}
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    email: event.target.value,
                  }))
                }
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="managed-password">
                Temporary password
              </FieldLabel>
              <Input
                id="managed-password"
                type="password"
                required
                minLength={8}
                value={form.temporary_password}
                onChange={(event) =>
                  setForm((current) => ({
                    ...current,
                    temporary_password: event.target.value,
                  }))
                }
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="managed-access">Access</FieldLabel>
              <Select
                items={INSTANCE_ROLE_ITEMS}
                value={form.role}
                onValueChange={(value) =>
                  setForm((current) => ({
                    ...current,
                    role: value as RoleValue,
                  }))
                }
              >
                <SelectTrigger id="managed-access">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={ROLE_USER}>User</SelectItem>
                  <SelectItem value="instance_admin">Instance admin</SelectItem>
                  {currentUserIsOwner && (
                    <SelectItem value="instance_owner">
                      Instance owner
                    </SelectItem>
                  )}
                </SelectContent>
              </Select>
            </Field>
          </FieldGroup>
          <Button type="submit" disabled={createMutation.isPending}>
            <ShieldCheck data-icon="inline-start" />
            {createMutation.isPending ? "Creating..." : "Create user"}
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  );
}
