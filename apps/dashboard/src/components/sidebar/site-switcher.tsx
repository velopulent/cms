import { useNavigate, useParams } from "@tanstack/react-router";
import { ChevronsUpDown, LayoutDashboard, Plus } from "lucide-react";
import type * as React from "react";
import { useState } from "react";

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  useSidebar,
} from "@/components/ui/sidebar";
import { Skeleton } from "@/components/ui/skeleton";
import { SiteAvatar } from "../site-avatar";

interface Site {
  name: string;
  id: string;
  icon: React.ReactNode;
  plan: string;
}

export function SiteSwitcher({
  sites,
  isLoading,
}: {
  sites: Site[];
  isLoading?: boolean;
}) {
  const { isMobile } = useSidebar();
  const navigate = useNavigate();
  const { siteId } = useParams({ from: "/_admin/sites/$siteId" });
  const [sidebarHovered, setSidebarHovered] = useState(false);
  const [hoveredSiteId, setHoveredSiteId] = useState<string | null>(null);

  const activeSite = sites.find((t) => t.id === siteId) ?? sites[0];

  if (isLoading && !activeSite) {
    return (
      <SidebarMenu>
        <SidebarMenuItem>
          <SidebarMenuButton size="lg">
            <Skeleton className="size-8 rounded-lg" />
            <div className="grid flex-1 gap-1">
              <Skeleton className="h-4 w-24" />
              <Skeleton className="h-3 w-12" />
            </div>
          </SidebarMenuButton>
        </SidebarMenuItem>
      </SidebarMenu>
    );
  }

  if (!activeSite) {
    return null;
  }

  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <DropdownMenu>
          <DropdownMenuTrigger
            render={
              <SidebarMenuButton
                size="lg"
                className="data-open:bg-sidebar-accent data-open:text-sidebar-accent-foreground"
                onMouseEnter={() => setSidebarHovered(true)}
                onMouseLeave={() => setSidebarHovered(false)}
              />
            }
          >
            <SiteAvatar siteName={activeSite.name} animate={sidebarHovered} />
            <div className="grid flex-1 text-left text-sm leading-tight">
              <span className="truncate font-medium">{activeSite.name}</span>
              <span className="truncate text-xs">{activeSite.plan}</span>
            </div>
            <ChevronsUpDown className="ml-auto" />
          </DropdownMenuTrigger>
          <DropdownMenuContent
            className="min-w-56 rounded-lg"
            align="start"
            side={isMobile ? "bottom" : "right"}
            sideOffset={4}
          >
            <DropdownMenuGroup>
              <DropdownMenuItem
                className="gap-2 p-2"
                onClick={() => navigate({ to: "/" })}
              >
                <div className="flex size-6 items-center justify-center rounded-md border">
                  <LayoutDashboard className="size-4" />
                </div>
                <div className="font-medium">Dashboard</div>
              </DropdownMenuItem>
            </DropdownMenuGroup>
            <DropdownMenuSeparator />
            <DropdownMenuGroup>
              <DropdownMenuLabel className="text-xs text-muted-foreground">
                Sites
              </DropdownMenuLabel>
              {sites.map((team) => (
                <DropdownMenuItem
                  key={team.id}
                  onMouseEnter={() => setHoveredSiteId(team.id)}
                  onMouseLeave={() => setHoveredSiteId(null)}
                  onClick={() =>
                    navigate({
                      to: "/sites/$siteId",
                      params: { siteId: team.id },
                    })
                  }
                  className="gap-3 px-3 py-2.5"
                >
                  <div className="flex size-6 items-center justify-center rounded-md border">
                    <SiteAvatar
                      siteName={team.name}
                      className="size-4"
                      animate={hoveredSiteId === team.id}
                    />
                  </div>
                  {team.name}
                </DropdownMenuItem>
              ))}
            </DropdownMenuGroup>
            <DropdownMenuSeparator />
            <DropdownMenuGroup>
              <DropdownMenuItem
                className="gap-2 p-2"
                onClick={() => navigate({ to: "/", search: { create: true } })}
              >
                <div className="flex size-6 items-center justify-center rounded-md border bg-transparent">
                  <Plus className="size-4" />
                </div>
                <div className="font-medium text-muted-foreground">
                  Add site
                </div>
              </DropdownMenuItem>
            </DropdownMenuGroup>
          </DropdownMenuContent>
        </DropdownMenu>
      </SidebarMenuItem>
    </SidebarMenu>
  );
}
