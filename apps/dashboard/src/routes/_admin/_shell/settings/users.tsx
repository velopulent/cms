import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { UserPlus } from "lucide-react";
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
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
  getInstanceUsers,
  getMe,
  type InstanceRole,
  instanceRoleLabel,
  isOperator,
  updateInstanceRole,
} from "@/lib/api";

export const Route = createFileRoute("/_admin/_shell/settings/users")({
  component: InstanceUsers,
});

const ROLE_USER = "user";
type RoleValue = InstanceRole | typeof ROLE_USER;

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

  const roleMutation = useMutation({
    mutationFn: ({
      userId,
      role,
    }: {
      userId: string;
      role: InstanceRole | null;
    }) => updateInstanceRole(userId, role),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["instance-users"] });
      toast.success("Instance role updated");
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
                    const targetIsOperator =
                      isOwner || user.instance_role === "instance_admin";
                    // Operators may change roles; only owners may touch an owner.
                    const canEditRole =
                      currentIsOperator && (currentIsOwner || !isOwner);
                    return (
                      <tr className="border-b last:border-0" key={user.id}>
                        <td className="p-3">
                          <div className="font-medium">{user.username}</div>
                          <div className="text-muted-foreground">
                            {user.email}
                          </div>
                        </td>
                        <td className="p-3">
                          <Badge
                            variant={targetIsOperator ? "default" : "secondary"}
                          >
                            {instanceRoleLabel(user.instance_role)}
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
                          {canEditRole && user.id !== me?.id ? (
                            <Select
                              value={user.instance_role ?? ROLE_USER}
                              onValueChange={(value) =>
                                roleMutation.mutate({
                                  userId: user.id,
                                  role: toRole(value as RoleValue),
                                })
                              }
                            >
                              <SelectTrigger className="ml-auto w-40">
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
    </div>
  );
}
