import { createContext, type ReactNode, useContext, useState } from "react";
import { logoutApi, type UserPublic } from "@/lib/api";

interface AuthContextValue {
  user: UserPublic | null;
  login: (user: UserPublic) => void;
  logout: () => Promise<void>;
  isAuthenticated: boolean;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<UserPublic | null>(() => {
    const stored = localStorage.getItem("cms_user");
    return stored ? JSON.parse(stored) : null;
  });

  const handleLogin = (newUser: UserPublic) => {
    localStorage.setItem("cms_user", JSON.stringify(newUser));
    setUser(newUser);
  };

  const handleLogout = async () => {
    try {
      await logoutApi();
    } catch {
      // Logout endpoint may fail if cookie is already expired — that's fine
    }
    localStorage.removeItem("cms_user");
    setUser(null);
  };

  return (
    <AuthContext.Provider
      value={{
        user,
        login: handleLogin,
        logout: handleLogout,
        isAuthenticated: !!user,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth must be used within AuthProvider");
  return ctx;
}
