import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { UserPlus } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { CreateUserDialog } from "@/components/instance/create-user-dialog";
import { UserCombobox } from "@/components/site-settings/user-combobox";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Field, FieldLabel } from "@/components/ui/field";
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
  getSiteMembers,
  inviteMember,
  isOperator,
  removeMember,
  siteRoleLabel,
  updateMemberRole,
} from "@/lib/api";

const ROLE_ITEMS = [
  { value: "editor", label: siteRoleLabel("editor") },
  { value: "viewer", label: siteRoleLabel("viewer") },
];

export function MembersSection({
  siteId,
  canManage,
}: {
  siteId: string;
  canManage: boolean;
}) {
  const queryClient = useQueryClient();
  const [selectedUserId, setSelectedUserId] = useState<string | null>(null);
  const [role, setRole] = useState<"editor" | "viewer">("editor");

  const { data: members, isLoading } = useQuery({
    queryKey: ["site-members", siteId],
    queryFn: () => getSiteMembers(siteId),
  });
  const { data: me } = useQuery({ queryKey: ["me"], queryFn: getMe });
  // Only operators may manage members and may read the instance user list.
  const { data: instanceUsers } = useQuery({
    queryKey: ["instance-users"],
    queryFn: getInstanceUsers,
    enabled: canManage,
  });

  const currentIsOwner = me?.instance_role === "instance_owner";
  const operators = (instanceUsers ?? []).filter((user) =>
    isOperator(user.instance_role),
  );
  const memberUserIds = new Set((members ?? []).map((m) => m.user_id));
  // Candidates exclude operators (they already have full access) and existing members.
  const candidates = (instanceUsers ?? []).filter(
    (user) => !isOperator(user.instance_role) && !memberUserIds.has(user.id),
  );

  const invalidate = () =>
    queryClient.invalidateQueries({ queryKey: ["site-members", siteId] });

  const inviteMutation = useMutation({
    mutationFn: (email: string) => inviteMember(siteId, { email, role }),
    onSuccess: () => {
      invalidate();
      setSelectedUserId(null);
      toast.success("Member added");
    },
    onError: (error: Error) => toast.error(error.message),
  });
  const roleMutation = useMutation({
    mutationFn: ({ userId, nextRole }: { userId: string; nextRole: string }) =>
      updateMemberRole(siteId, userId, nextRole),
    onSuccess: () => {
      invalidate();
      toast.success("Role updated");
    },
    onError: (error: Error) => toast.error(error.message),
  });
  const removeMutation = useMutation({
    mutationFn: (userId: string) => removeMember(siteId, userId),
    onSuccess: () => {
      invalidate();
      toast.success("Member removed");
    },
    onError: (error: Error) => toast.error(error.message),
  });

  const handleAdd = () => {
    const user = candidates.find(
      (candidate) => candidate.id === selectedUserId,
    );
    if (user) inviteMutation.mutate(user.email);
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>Members</CardTitle>
        <CardDescription>
          Editors create and edit content and files. Viewers have read-only
          access. Instance operators have full access to every site and cannot
          be added as members.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-5">
        {canManage && (
          <div className="flex flex-col gap-3 rounded-lg border bg-muted/30 p-3 sm:flex-row sm:items-end">
            <Field className="flex-1">
              <FieldLabel>Add a member</FieldLabel>
              <UserCombobox
                users={candidates}
                value={selectedUserId}
                onChange={setSelectedUserId}
                placeholder="Select a user…"
                emptyText="No users left to add."
                footer={
                  <CreateUserDialog
                    currentUserIsOwner={currentIsOwner}
                    onCreated={(user) => setSelectedUserId(user.id)}
                    trigger={
                      <Button
                        type="button"
                        variant="ghost"
                        className="w-full justify-start"
                      >
                        <UserPlus data-icon="inline-start" />
                        Create new user
                      </Button>
                    }
                  />
                }
              />
            </Field>
            <Field className="sm:w-40">
              <FieldLabel htmlFor="member-role">Role</FieldLabel>
              <Select
                items={ROLE_ITEMS}
                value={role}
                onValueChange={(value) => setRole(value as "editor" | "viewer")}
              >
                <SelectTrigger id="member-role">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="editor">Editor</SelectItem>
                  <SelectItem value="viewer">Viewer</SelectItem>
                </SelectContent>
              </Select>
            </Field>
            <Button
              type="button"
              onClick={handleAdd}
              disabled={!selectedUserId || inviteMutation.isPending}
            >
              Add member
            </Button>
          </div>
        )}

        {isLoading ? (
          <Skeleton className="h-24 w-full" />
        ) : (
          <div className="overflow-x-auto rounded-md border">
            <table className="w-full min-w-2xl text-sm">
              <thead className="border-b bg-muted/50 text-left">
                <tr>
                  <th className="p-3 font-medium">Member</th>
                  <th className="p-3 font-medium">Role</th>
                  <th className="p-3 text-right font-medium">Actions</th>
                </tr>
              </thead>
              <tbody>
                {operators.map((user) => (
                  <tr className="border-b last:border-0" key={user.id}>
                    <td className="p-3">
                      <div className="font-medium">{user.name}</div>
                      <div className="text-muted-foreground">{user.email}</div>
                    </td>
                    <td className="p-3">
                      <Badge>{siteRoleLabel(user.instance_role)}</Badge>
                    </td>
                    <td className="p-3 text-right text-xs text-muted-foreground">
                      Full access
                    </td>
                  </tr>
                ))}
                {members?.map((member) => (
                  <tr className="border-b last:border-0" key={member.id}>
                    <td className="p-3">
                      <div className="font-medium">{member.name}</div>
                      <div className="text-muted-foreground">
                        {member.email}
                      </div>
                    </td>
                    <td className="p-3">
                      {canManage ? (
                        <Select
                          items={ROLE_ITEMS}
                          value={member.role}
                          onValueChange={(nextRole) => {
                            if (nextRole) {
                              roleMutation.mutate({
                                userId: member.user_id,
                                nextRole,
                              });
                            }
                          }}
                        >
                          <SelectTrigger className="w-36">
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            <SelectItem value="editor">Editor</SelectItem>
                            <SelectItem value="viewer">Viewer</SelectItem>
                          </SelectContent>
                        </Select>
                      ) : (
                        <Badge variant="secondary">
                          {siteRoleLabel(member.role)}
                        </Badge>
                      )}
                    </td>
                    <td className="p-3 text-right">
                      {canManage && (
                        <Button
                          variant="ghost"
                          size="sm"
                          disabled={removeMutation.isPending}
                          onClick={() => removeMutation.mutate(member.user_id)}
                        >
                          Remove
                        </Button>
                      )}
                    </td>
                  </tr>
                ))}
                {operators.length === 0 && (members?.length ?? 0) === 0 && (
                  <tr>
                    <td
                      className="p-6 text-center text-muted-foreground"
                      colSpan={3}
                    >
                      No members yet.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
