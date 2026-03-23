import { useQuery } from "@tanstack/react-query";
import {
  createFileRoute,
  Link,
  Outlet,
  redirect,
} from "@tanstack/react-router";
import { FileText, Layers, LayoutDashboard, LogOut } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { useAuth } from "@/contexts/auth-context";
import { type ContentType, getContentTypes } from "@/lib/api";

export const Route = createFileRoute("/_admin")({
  beforeLoad: () => {
    const token = localStorage.getItem("cms_token");
    if (!token) {
      throw redirect({ to: "/login" });
    }
  },
  component: AdminLayout,
});

function AdminLayout() {
  const auth = useAuth();
  const { data: contentTypes } = useQuery({
    queryKey: ["content-types"],
    queryFn: getContentTypes,
  });

  return (
    <div className="flex h-dvh">
      <aside className="flex w-64 shrink-0 flex-col border-r bg-sidebar">
        <div className="flex h-14 items-center px-4 font-semibold">CMS</div>
        <Separator />
        <nav className="flex-1 overflow-y-auto p-2">
          <div className="flex flex-col gap-1">
            <Button
              variant="ghost"
              className="justify-start"
              render={<Link to="/" />}
            >
              <LayoutDashboard data-icon="inline-start" />
              Dashboard
            </Button>
            <Button
              variant="ghost"
              className="justify-start"
              render={<Link to="/content-types" />}
            >
              <Layers data-icon="inline-start" />
              Content Types
            </Button>
          </div>

          {contentTypes && contentTypes.length > 0 && (
            <>
              <Separator className="my-2" />
              <p className="px-2 py-1 text-xs font-medium text-muted-foreground">
                Content
              </p>
              <div className="flex flex-col gap-1">
                {contentTypes.map((ct: ContentType) => (
                  <Button
                    key={ct.id}
                    variant="ghost"
                    className="justify-start"
                    render={
                      <Link
                        to="/content/$typeSlug"
                        params={{ typeSlug: ct.slug }}
                      />
                    }
                  >
                    <FileText data-icon="inline-start" />
                    {ct.name}
                  </Button>
                ))}
              </div>
            </>
          )}
        </nav>
        <Separator />
        <div className="flex items-center justify-between p-3">
          <span className="truncate text-sm text-muted-foreground">
            {auth.user?.username}
          </span>
          <Button variant="ghost" size="icon-sm" onClick={auth.logout}>
            <LogOut />
          </Button>
        </div>
      </aside>
      <main className="flex-1 overflow-y-auto">
        <Outlet />
      </main>
    </div>
  );
}
