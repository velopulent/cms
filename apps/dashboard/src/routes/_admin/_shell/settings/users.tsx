import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { MoreHorizontal, UserPlus } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { CreateUserDialog } from "@/components/instance/create-user-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
  adminSetUserPassword,
  deleteUser,
  getInstanceUsers,
  getMe,
  INSTANCE_ROLE_ITEMS,
  type InstanceRole,
  instanceRoleLabel,
  isOperator,
  ROLE_USER,
  type RoleValue,
  type UserPublic,
  updateInstanceRole,
  updateUser,
} from "@/lib/api";

export const Route = createFileRoute("/_admin/_shell/settings/users")({
  component: InstanceUsers,
});

function toRole(value: RoleValue): InstanceRole | null {
  return value === ROLE_USER ? null : value;
}

function InstanceUsers() {
  const queryClient = useQueryClient();
  const { data: me } = useQuery({ queryKey: ["me"], queryFn: getMe });
  const currentIsOwner = me?.instance_role === "instance_owner";
  const currentIsOperator = isOperator(me?.instance_role);
  const { data: users, isLoading } = useQuery({
    queryKey: ["instance-users"],
    queryFn: getInstanceUsers,
  });

  const [editUser, setEditUser] = useState<UserPublic | null>(null);
  const [passwordUser, setPasswordUser] = useState<UserPublic | null>(null);
  const [removeUser, setRemoveUser] = useState<UserPublic | null>(null);

  const invalidate = () =>
    queryClient.invalidateQueries({ queryKey: ["instance-users"] });

  const roleMutation = useMutation({
    mutationFn: ({
      userId,
      role,
    }: {
      userId: string;
      role: InstanceRole | null;
    }) => updateInstanceRole(userId, role),
    onSuccess: () => {
      invalidate();
      toast.success("Instance role updated");
    },
    onError: (error: Error) => toast.error(error.message),
  });

  const removeMutation = useMutation({
    mutationFn: (userId: string) => deleteUser(userId),
    onSuccess: () => {
      invalidate();
      setRemoveUser(null);
      toast.success("User deleted");
    },
    onError: (error: Error) => toast.error(error.message),
  });

  return (
    <div className="flex flex-col gap-4">
      <Card>
        <CardHeader>
          <CardTitle>Users</CardTitle>

          <CardDescription>
            Instance operators (owners and admins) can create and manage sites.
            Only owners can grant the owner role.
          </CardDescription>
          <CardAction>
            <CreateUserDialog
              currentUserIsOwner={currentIsOwner}
              trigger={
                <Button>
                  <UserPlus data-icon="inline-start" />
                  Create user
                </Button>
              }
            />
          </CardAction>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="flex flex-col gap-3">
              <Skeleton className="h-16 w-full" />
              <Skeleton className="h-16 w-full" />
            </div>
          ) : (
            <div className="overflow-x-auto rounded-md border">
              <table className="w-full min-w-2xl text-sm">
                <thead className="border-b bg-muted/50 text-left">
                  <tr>
                    <th className="p-3 font-medium">User</th>
                    <th className="p-3 font-medium">Access</th>
                    <th className="p-3 font-medium">Password</th>
                    <th className="p-3 text-right font-medium">Action</th>
                  </tr>
                </thead>
                <tbody>
                  {users?.map((user) => {
                    const isOwner = user.instance_role === "instance_owner";
                    const isSelf = user.id === me?.id;
                    // Operators may manage users; only owners may touch an owner.
                    const canManage =
                      currentIsOperator && (currentIsOwner || !isOwner);
                    const canEditRole = canManage && !isSelf;
                    return (
                      <tr className="border-b last:border-0" key={user.id}>
                        <td className="p-3">
                          <div className="font-medium">{user.name}</div>
                          <div className="text-muted-foreground">
                            {user.email}
                          </div>
                        </td>
                        <td className="p-3">
                          {canEditRole ? (
                            <Select
                              items={INSTANCE_ROLE_ITEMS}
                              value={user.instance_role ?? ROLE_USER}
                              onValueChange={(value) =>
                                roleMutation.mutate({
                                  userId: user.id,
                                  role: toRole(value as RoleValue),
                                })
                              }
                            >
                              <SelectTrigger className="w-40">
                                <SelectValue />
                              </SelectTrigger>
                              <SelectContent>
                                <SelectItem value={ROLE_USER}>User</SelectItem>
                                <SelectItem value="instance_admin">
                                  Instance admin
                                </SelectItem>
                                {currentIsOwner && (
                                  <SelectItem value="instance_owner">
                                    Instance owner
                                  </SelectItem>
                                )}
                              </SelectContent>
                            </Select>
                          ) : (
                            <Badge variant={isOwner ? "default" : "secondary"}>
                              {instanceRoleLabel(user.instance_role)}
                            </Badge>
                          )}
                        </td>
                        <td className="p-3">
                          {user.must_change_password ? (
                            <Badge variant="outline">Change required</Badge>
                          ) : (
                            "Configured"
                          )}
                        </td>
                        <td className="p-3 text-right">
                          {canManage ? (
                            <DropdownMenu>
                              <DropdownMenuTrigger
                                render={
                                  <Button
                                    variant="ghost"
                                    size="icon"
                                    className="ml-auto"
                                  />
                                }
                              >
                                <MoreHorizontal />
                                <span className="sr-only">Manage user</span>
                              </DropdownMenuTrigger>
                              <DropdownMenuContent align="end">
                                <DropdownMenuItem
                                  onClick={() => setEditUser(user)}
                                >
                                  Edit details
                                </DropdownMenuItem>
                                <DropdownMenuItem
                                  onClick={() => setPasswordUser(user)}
                                >
                                  Reset password
                                </DropdownMenuItem>
                                {!isSelf && (
                                  <>
                                    <DropdownMenuSeparator />
                                    <DropdownMenuItem
                                      variant="destructive"
                                      onClick={() => setRemoveUser(user)}
                                    >
                                      Delete user
                                    </DropdownMenuItem>
                                  </>
                                )}
                              </DropdownMenuContent>
                            </DropdownMenu>
                          ) : (
                            <span className="text-muted-foreground">—</span>
                          )}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>

      <EditUserDialog
        user={editUser}
        onOpenChange={(open) => !open && setEditUser(null)}
        onSaved={invalidate}
      />
      <ResetPasswordDialog
        user={passwordUser}
        onOpenChange={(open) => !open && setPasswordUser(null)}
      />
      <Dialog
        open={!!removeUser}
        onOpenChange={(open) => !open && setRemoveUser(null)}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete user</DialogTitle>
            <DialogDescription>
              Permanently delete <strong>{removeUser?.name}</strong> (
              {removeUser?.email}). This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <DialogClose render={<Button variant="outline" />}>
              Cancel
            </DialogClose>
            <Button
              variant="destructive"
              disabled={removeMutation.isPending}
              onClick={() => removeUser && removeMutation.mutate(removeUser.id)}
            >
              {removeMutation.isPending ? "Deleting..." : "Delete user"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function EditUserDialog({
  user,
  onOpenChange,
  onSaved,
}: {
  user: UserPublic | null;
  onOpenChange: (open: boolean) => void;
  onSaved: () => void;
}) {
  const [name, setUsername] = useState("");
  const [email, setEmail] = useState("");

  const mutation = useMutation({
    mutationFn: () => {
      if (!user) throw new Error("No user selected");
      return updateUser(user.id, { name, email });
    },
    onSuccess: () => {
      onSaved();
      onOpenChange(false);
      toast.success("User updated");
    },
    onError: (error: Error) => toast.error(error.message),
  });

  return (
    <Dialog
      open={!!user}
      onOpenChange={(open) => {
        if (open && user) {
          setUsername(user.name);
          setEmail(user.email);
        }
        onOpenChange(open);
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Edit user</DialogTitle>
          <DialogDescription>
            Update this user's display name and email.
          </DialogDescription>
        </DialogHeader>
        <form
          className="flex flex-col gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            mutation.mutate();
          }}
        >
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="edit-name">Name</FieldLabel>
              <Input
                id="edit-name"
                required
                minLength={1}
                value={name}
                onChange={(event) => setUsername(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="edit-email">Email</FieldLabel>
              <Input
                id="edit-email"
                type="email"
                required
                value={email}
                onChange={(event) => setEmail(event.target.value)}
              />
            </Field>
          </FieldGroup>
          <DialogFooter>
            <DialogClose render={<Button type="button" variant="outline" />}>
              Cancel
            </DialogClose>
            <Button type="submit" disabled={mutation.isPending}>
              {mutation.isPending ? "Saving..." : "Save changes"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function ResetPasswordDialog({
  user,
  onOpenChange,
}: {
  user: UserPublic | null;
  onOpenChange: (open: boolean) => void;
}) {
  const [password, setPassword] = useState("");

  const mutation = useMutation({
    mutationFn: () => {
      if (!user) throw new Error("No user selected");
      return adminSetUserPassword(user.id, password);
    },
    onSuccess: () => {
      onOpenChange(false);
      toast.success("Password reset. The user must change it on next sign in.");
    },
    onError: (error: Error) => toast.error(error.message),
  });

  return (
    <Dialog
      open={!!user}
      onOpenChange={(open) => {
        if (open) setPassword("");
        onOpenChange(open);
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Reset password</DialogTitle>
          <DialogDescription>
            Set a temporary password for <strong>{user?.name}</strong>. They
            must change it after signing in.
          </DialogDescription>
        </DialogHeader>
        <form
          className="flex flex-col gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            mutation.mutate();
          }}
        >
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="reset-password">
                Temporary password
              </FieldLabel>
              <Input
                id="reset-password"
                type="password"
                required
                minLength={8}
                value={password}
                onChange={(event) => setPassword(event.target.value)}
              />
            </Field>
          </FieldGroup>
          <DialogFooter>
            <DialogClose render={<Button type="button" variant="outline" />}>
              Cancel
            </DialogClose>
            <Button type="submit" disabled={mutation.isPending}>
              {mutation.isPending ? "Saving..." : "Reset password"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
