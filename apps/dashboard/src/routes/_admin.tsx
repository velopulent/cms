import { createFileRoute, Outlet, redirect } from "@tanstack/react-router";
import { ApiError, getMe } from "@/lib/api";

export const Route = createFileRoute("/_admin")({
  beforeLoad: async ({ context, location }) => {
    try {
      const user = await context.queryClient.ensureQueryData({
        queryKey: ["me"],
        queryFn: getMe,
      });
      if (user.must_change_password && location.pathname !== "/account") {
        throw redirect({ to: "/account" });
      }
    } catch (err: any) {
      // Only redirect on auth failure
      if (err instanceof ApiError && err.status === 401) {
        throw redirect({ to: "/login" });
      }

      // Let other errors bubble (network/server issues)
      throw err;
    }
  },
  component: () => <Outlet />,
});
