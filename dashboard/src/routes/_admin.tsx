import { createFileRoute, Outlet, redirect } from "@tanstack/react-router";
import { ApiError, getMe } from "@/lib/api";

export const Route = createFileRoute("/_admin")({
  beforeLoad: async ({ context }) => {
    try {
      await context.queryClient.ensureQueryData({
        queryKey: ["me"],
        queryFn: getMe,
      });
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
