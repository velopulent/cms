import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { KeyRound, LogOut } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { PersonalTokensCard } from "@/components/account/personal-tokens-card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { useAuth } from "@/contexts/auth-context";
import {
  changePassword,
  getSessions,
  revokeAllSessions,
  updateMyProfile,
} from "@/lib/api";

export const Route = createFileRoute("/_admin/_shell/account")({
  component: AccountPage,
});

function AccountPage() {
  const auth = useAuth();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [displayName, setDisplayName] = useState(auth.user?.name ?? "");
  const { data: sessions, isLoading } = useQuery({
    queryKey: ["sessions"],
    queryFn: getSessions,
  });

  // Sync the field once the signed-in user loads (auth.user may be null on first render).
  const loadedName = auth.user?.name;
  useEffect(() => {
    if (loadedName) setDisplayName(loadedName);
  }, [loadedName]);

  const profileMutation = useMutation({
    mutationFn: () => updateMyProfile({ name: displayName }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["me"] });
      toast.success("Profile updated");
    },
    onError: (error: Error) => toast.error(error.message),
  });

  const passwordMutation = useMutation({
    mutationFn: () => changePassword(currentPassword, newPassword),
    onSuccess: async () => {
      toast.success("Password changed. Sign in again.");
      // Keep the ["me"] query alive (clearing it orphans AuthProvider's
      // observer); refetch it instead → 401 → signed-out state.
      queryClient.removeQueries({ predicate: (q) => q.queryKey[0] !== "me" });
      await queryClient.refetchQueries({ queryKey: ["me"] });
      navigate({ to: "/login" });
    },
    onError: (error: Error) => toast.error(error.message),
  });

  const revokeMutation = useMutation({
    mutationFn: revokeAllSessions,
    onSuccess: async () => {
      toast.success("All sessions revoked");
      // Keep the ["me"] query alive (clearing it orphans AuthProvider's
      // observer); refetch it instead → 401 → signed-out state.
      queryClient.removeQueries({ predicate: (q) => q.queryKey[0] !== "me" });
      await queryClient.refetchQueries({ queryKey: ["me"] });
      navigate({ to: "/login" });
    },
    onError: (error: Error) => toast.error(error.message),
  });

  return (
    <main className="mx-auto flex w-full max-w-3xl flex-col gap-6 p-4 sm:p-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">
          Account security
        </h1>
        <p className="text-sm text-muted-foreground">
          Manage your password and signed-in sessions.
        </p>
      </div>

      {auth.user?.must_change_password ? (
        <Card className="border-amber-500/40 bg-amber-500/5">
          <CardContent className="flex gap-3 pt-6">
            <KeyRound className="mt-0.5 size-5 text-amber-700" />
            <div>
              <p className="font-medium">Password change required</p>
              <p className="text-sm text-muted-foreground">
                Set a private password before continuing to the dashboard.
              </p>
            </div>
          </CardContent>
        </Card>
      ) : null}

      <Card>
        <CardHeader>
          <CardTitle>Profile</CardTitle>
          <CardDescription>
            Your display name is shown across the dashboard. You sign in with
            your email.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form
            className="space-y-5"
            onSubmit={(event) => {
              event.preventDefault();
              profileMutation.mutate();
            }}
          >
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="display-name">Name</FieldLabel>
                <Input
                  id="display-name"
                  value={displayName}
                  onChange={(event) => setDisplayName(event.target.value)}
                  placeholder="John Doe"
                  required
                />
              </Field>
              {auth.user?.email ? (
                <Field>
                  <FieldLabel htmlFor="account-email">Email</FieldLabel>
                  <Input id="account-email" value={auth.user.email} disabled />
                </Field>
              ) : null}
            </FieldGroup>
            <Button
              type="submit"
              disabled={
                profileMutation.isPending ||
                displayName.trim().length === 0 ||
                displayName === auth.user?.name
              }
            >
              {profileMutation.isPending ? "Saving..." : "Save profile"}
            </Button>
          </form>
        </CardContent>
      </Card>

      <PersonalTokensCard />

      <Card>
        <CardHeader>
          <CardTitle>Change password</CardTitle>
          <CardDescription>
            Changing your password revokes every active session.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form
            className="space-y-5"
            onSubmit={(event) => {
              event.preventDefault();
              passwordMutation.mutate();
            }}
          >
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="current-password">
                  Current password
                </FieldLabel>
                <Input
                  id="current-password"
                  type="password"
                  autoComplete="current-password"
                  value={currentPassword}
                  onChange={(event) => setCurrentPassword(event.target.value)}
                  required
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="new-password">New password</FieldLabel>
                <Input
                  id="new-password"
                  type="password"
                  autoComplete="new-password"
                  minLength={8}
                  value={newPassword}
                  onChange={(event) => setNewPassword(event.target.value)}
                  required
                />
              </Field>
            </FieldGroup>
            <Button type="submit" disabled={passwordMutation.isPending}>
              {passwordMutation.isPending ? "Changing..." : "Change password"}
            </Button>
          </form>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Active sessions</CardTitle>
          <CardDescription>
            Review session activity or sign out everywhere.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {isLoading ? (
            <div className="space-y-2">
              <Skeleton className="h-12 w-full" />
              <Skeleton className="h-12 w-full" />
            </div>
          ) : (
            <div className="divide-y rounded-md border">
              {sessions?.map((session) => (
                <div
                  key={session.id}
                  className="flex flex-col gap-1 p-3 sm:flex-row sm:items-center sm:justify-between"
                >
                  <div>
                    <p className="text-sm font-medium">
                      Last active{" "}
                      {new Date(session.last_seen_at).toLocaleString()}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      Expires {new Date(session.expires_at).toLocaleString()}
                    </p>
                  </div>
                  {session.current ? (
                    <Badge variant="secondary">Current</Badge>
                  ) : null}
                </div>
              ))}
            </div>
          )}
          <Button
            variant="destructive"
            onClick={() => revokeMutation.mutate()}
            disabled={revokeMutation.isPending}
          >
            <LogOut />
            Revoke all sessions
          </Button>
        </CardContent>
      </Card>
    </main>
  );
}
