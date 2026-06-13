import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { ShieldCheck, UserPlus } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import {
  createManagedUser,
  getInstanceUsers,
  updateInstanceRole,
} from "@/lib/api";

export const Route = createFileRoute("/_admin/_shell/settings/users")({
  component: InstanceUsers,
});

function InstanceUsers() {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const [form, setForm] = useState({
    username: "",
    email: "",
    temporary_password: "",
    instance_owner: false,
  });
  const { data: users, isLoading } = useQuery({
    queryKey: ["instance-users"],
    queryFn: getInstanceUsers,
  });

  const createMutation = useMutation({
    mutationFn: createManagedUser,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["instance-users"] });
      setOpen(false);
      setForm({
        username: "",
        email: "",
        temporary_password: "",
        instance_owner: false,
      });
      toast.success("User created");
    },
    onError: (error: Error) => toast.error(error.message),
  });

  const roleMutation = useMutation({
    mutationFn: ({
      userId,
      instanceOwner,
    }: {
      userId: string;
      instanceOwner: boolean;
    }) => updateInstanceRole(userId, instanceOwner),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["instance-users"] });
      toast.success("Instance role updated");
    },
    onError: (error: Error) => toast.error(error.message),
  });

  return (
    <div className="flex flex-col gap-4">
      <div className="flex justify-end">
        <Button onClick={() => setOpen(true)}>
          <UserPlus data-icon="inline-start" />
          Create user
        </Button>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Users</CardTitle>
          <CardDescription>
            Instance owners can create sites and manage installation-wide
            settings.
          </CardDescription>
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
                    return (
                      <tr className="border-b last:border-0" key={user.id}>
                        <td className="p-3">
                          <div className="font-medium">{user.username}</div>
                          <div className="text-muted-foreground">
                            {user.email}
                          </div>
                        </td>
                        <td className="p-3">
                          <Badge variant={isOwner ? "default" : "secondary"}>
                            {isOwner ? "Instance owner" : "User"}
                          </Badge>
                        </td>
                        <td className="p-3">
                          {user.must_change_password ? (
                            <Badge variant="outline">Change required</Badge>
                          ) : (
                            "Configured"
                          )}
                        </td>
                        <td className="p-3 text-right">
                          <Button
                            variant="outline"
                            size="sm"
                            disabled={roleMutation.isPending}
                            onClick={() =>
                              roleMutation.mutate({
                                userId: user.id,
                                instanceOwner: !isOwner,
                              })
                            }
                          >
                            {isOwner ? "Remove owner" : "Make owner"}
                          </Button>
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

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Create managed user</DialogTitle>
            <DialogDescription>
              User must change temporary password after signing in.
            </DialogDescription>
          </DialogHeader>
          <form
            className="flex flex-col gap-4"
            onSubmit={(event) => {
              event.preventDefault();
              createMutation.mutate(form);
            }}
          >
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="managed-username">Username</FieldLabel>
                <Input
                  id="managed-username"
                  required
                  minLength={3}
                  value={form.username}
                  onChange={(event) =>
                    setForm((current) => ({
                      ...current,
                      username: event.target.value,
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
                <FieldLabel htmlFor="temporary-password">
                  Temporary password
                </FieldLabel>
                <Input
                  id="temporary-password"
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
              <Field orientation="horizontal">
                <Checkbox
                  id="instance-owner"
                  checked={form.instance_owner}
                  onCheckedChange={(checked) =>
                    setForm((current) => ({
                      ...current,
                      instance_owner: checked === true,
                    }))
                  }
                />
                <FieldLabel htmlFor="instance-owner">
                  Grant instance owner access
                </FieldLabel>
              </Field>
            </FieldGroup>
            <Button type="submit" disabled={createMutation.isPending}>
              <ShieldCheck data-icon="inline-start" />
              {createMutation.isPending ? "Creating..." : "Create user"}
            </Button>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}
