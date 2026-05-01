import { createFileRoute, Outlet } from "@tanstack/react-router";
import { AppSidebar } from "@/components/sidebar/app-sidebar";
import { AppBreadcrumb } from "@/components/app-breadcrumb";
import { Separator } from "@/components/ui/separator";
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar";
import { ModeToggle } from "@/components/theme-toggle";

export const Route = createFileRoute("/_admin/sites/$siteId")({
  component: SiteLayout,
});

function SiteLayout() {
  return (
    <SidebarProvider>
      <AppSidebar />
      <SidebarInset>
        <header className="flex w-full justify-between px-4 h-16 shrink-0 items-center gap-2 transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-12">
          <div className="flex items-center gap-2">
            <SidebarTrigger className="-ml-1" />
            <Separator
              orientation="vertical"
              className="mr-2 data-vertical:h-4 data-vertical:self-auto"
            />
            <AppBreadcrumb />
          </div>
          <ModeToggle />
        </header>
        <Outlet />
      </SidebarInset>
    </SidebarProvider>
  );
}