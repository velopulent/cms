import { createFileRoute, Outlet, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_admin")({
  beforeLoad: () => {
    const stored = localStorage.getItem("cms_user");
    if (!stored) {
      throw redirect({ to: "/login" });
    }
  },
  component: () => <Outlet />,
});
