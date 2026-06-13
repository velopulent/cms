import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { toast } from "sonner";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
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
  getSiteMembers,
  inviteMember,
  removeMember,
  transferOwnership,
  updateMemberRole,
} from "@/lib/api";

export function MembersSection({
  siteId,
  currentRole,
}: {
  siteId: string;
  currentRole: "owner" | "admin" | "editor" | "viewer";
}) {
  const queryClient = useQueryClient();
  const [username, setUsername] = useState("");
  const [role, setRole] = useState<"admin" | "editor" | "viewer">("viewer");
  const { data: members, isLoading } = useQuery({
    queryKey: ["site-members", siteId],
    queryFn: () => getSiteMembers(siteId),
  });
  const canManage = currentRole === "owner" || currentRole === "admin";

  const invalidate = () =>
    queryClient.invalidateQueries({ queryKey: ["site-members", siteId] });
  const inviteMutation = useMutation({
    mutationFn: () => inviteMember(siteId, { username, role }),
    onSuccess: () => {
      invalidate();
      setUsername("");
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
  const transferMutation = useMutation({
    mutationFn: (userId: string) => transferOwnership(siteId, userId),
    onSuccess: () => {
      invalidate();
      queryClient.invalidateQueries({ queryKey: ["sites"] });
      toast.success("Ownership transferred");
    },
    onError: (error: Error) => toast.error(error.message),
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle>Members and ownership</CardTitle>
        <CardDescription>
          Editors manage content and files. Admins also manage schemas,
          webhooks, keys, and site settings.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {canManage && (
          <form
            className="flex flex-col gap-3 sm:flex-row sm:items-end"
            onSubmit={(event) => {
              event.preventDefault();
              inviteMutation.mutate();
            }}
          >
            <Field className="flex-1">
              <FieldLabel htmlFor="member-username">Username</FieldLabel>
              <Input
                id="member-username"
                value={username}
                onChange={(event) => setUsername(event.target.value)}
                required
              />
            </Field>
            <Field className="sm:w-44">
              <FieldLabel htmlFor="member-role">Role</FieldLabel>
              <Select
                value={role}
                onValueChange={(value) =>
                  setRole(value as "admin" | "editor" | "viewer")
                }
              >
                <SelectTrigger id="member-role">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {currentRole === "owner" && (
                    <SelectItem value="admin">Admin</SelectItem>
                  )}
                  <SelectItem value="editor">Editor</SelectItem>
                  <SelectItem value="viewer">Viewer</SelectItem>
                </SelectContent>
              </Select>
            </Field>
            <Button type="submit" disabled={inviteMutation.isPending}>
              Add member
            </Button>
          </form>
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
                {members?.map((member) => {
                  const targetIsAdmin = member.role === "admin";
                  const editable =
                    canManage &&
                    member.role !== "owner" &&
                    (!targetIsAdmin || currentRole === "owner");
                  return (
                    <tr className="border-b last:border-0" key={member.id}>
                      <td className="p-3">
                        <div className="font-medium">{member.username}</div>
                        <div className="text-muted-foreground">
                          {member.email}
                        </div>
                      </td>
                      <td className="p-3">
                        {editable ? (
                          <Select
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
                              {currentRole === "owner" && (
                                <SelectItem value="admin">Admin</SelectItem>
                              )}
                              <SelectItem value="editor">Editor</SelectItem>
                              <SelectItem value="viewer">Viewer</SelectItem>
                            </SelectContent>
                          </Select>
                        ) : (
                          <Badge variant="secondary">{member.role}</Badge>
                        )}
                      </td>
                      <td className="p-3 text-right">
                        <div className="flex justify-end gap-2">
                          {currentRole === "owner" &&
                            member.role !== "owner" && (
                              <AlertDialog>
                                <AlertDialogTrigger
                                  render={
                                    <Button variant="outline" size="sm" />
                                  }
                                >
                                  Transfer ownership
                                </AlertDialogTrigger>
                                <AlertDialogContent>
                                  <AlertDialogHeader>
                                    <AlertDialogTitle>
                                      Transfer site ownership?
                                    </AlertDialogTitle>
                                    <AlertDialogDescription>
                                      {member.username} becomes owner. Your role
                                      becomes admin.
                                    </AlertDialogDescription>
                                  </AlertDialogHeader>
                                  <AlertDialogFooter>
                                    <AlertDialogCancel>
                                      Cancel
                                    </AlertDialogCancel>
                                    <AlertDialogAction
                                      onClick={() =>
                                        transferMutation.mutate(member.user_id)
                                      }
                                    >
                                      Transfer ownership
                                    </AlertDialogAction>
                                  </AlertDialogFooter>
                                </AlertDialogContent>
                              </AlertDialog>
                            )}
                          {editable && (
                            <Button
                              variant="ghost"
                              size="sm"
                              disabled={removeMutation.isPending}
                              onClick={() =>
                                removeMutation.mutate(member.user_id)
                              }
                            >
                              Remove
                            </Button>
                          )}
                        </div>
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
  );
}
