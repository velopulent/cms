import { createFileRoute, Outlet, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/_admin")({
  beforeLoad: () => {
    const token = localStorage.getItem("cms_token");
    if (!token) {
      throw redirect({ to: "/login" });
    }
  },
  component: () => <Outlet />,
});
