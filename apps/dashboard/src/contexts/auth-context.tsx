import { useQuery, useQueryClient } from "@tanstack/react-query";
import { createContext, type ReactNode, useContext } from "react";
import { getMe, logoutApi, type UserPublic } from "@/lib/api";

interface AuthContextValue {
  user: UserPublic | null;
  login: () => Promise<void>;
  logout: () => Promise<void>;
  isAuthenticated: boolean;
  isLoading: boolean;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient();

  const { data: user, isLoading } = useQuery({
    queryKey: ["me"],
    queryFn: getMe,
    retry: false,
    staleTime: 5 * 60 * 1000, // cache for 5 mins
  });

  const handleLogin = async () => {
    // A previous user's data may still be cached. Drop every query EXCEPT
    // ["me"]: that query is watched by this provider's always-mounted observer,
    // and removing it (clear/removeQueries) orphans the observer — it never
    // refetches, so the UI keeps showing the previous user until a hard reload.
    // Instead refetch ["me"] in place so the provider re-renders with the
    // freshly signed-in user.
    queryClient.removeQueries({ predicate: (q) => q.queryKey[0] !== "me" });
    await queryClient.refetchQueries({ queryKey: ["me"] });
  };

  const handleLogout = async () => {
    try {
      await logoutApi();
    } catch {
      // ignore errors (expired session etc.)
    }

    // Same reasoning as handleLogin: purge all other user-scoped queries
    // (sites/sessions/collections/files…) but keep + refetch ["me"] so the
    // observer stays attached. The refetch hits the now-invalid session →
    // 401 → user becomes null.
    queryClient.removeQueries({ predicate: (q) => q.queryKey[0] !== "me" });
    await queryClient.refetchQueries({ queryKey: ["me"] });
  };

  return (
    <AuthContext.Provider
      value={{
        user: user ?? null,
        login: handleLogin,
        logout: handleLogout,
        isAuthenticated: !!user,
        isLoading,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error("useAuth must be used within AuthProvider");
  }
  return ctx;
}
