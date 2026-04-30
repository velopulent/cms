import { createContext, type ReactNode, useContext } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { logoutApi, getMe, type UserPublic } from "@/lib/api";

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
    // After successful login API call elsewhere
    await queryClient.invalidateQueries({ queryKey: ["me"] });
  };

  const handleLogout = async () => {
    try {
      await logoutApi();
    } catch {
      // ignore errors (expired session etc.)
    }

    queryClient.removeQueries({ queryKey: ["me"] });
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
