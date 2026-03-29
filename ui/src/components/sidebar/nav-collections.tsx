"use client";

import { Link, useRouterState } from "@tanstack/react-router";
import {
  SidebarGroup,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSkeleton,
} from "@/components/ui/sidebar";

export function NavCollections({
  collections,
  isLoading,
}: {
  collections: {
    name: string;
    url: string;
    icon: React.ReactNode;
  }[];
  isLoading?: boolean;
}) {
  const pathname = useRouterState({ select: (s) => s.location.pathname });

  return (
    <SidebarGroup className="group-data-[collapsible=icon]:hidden">
      <SidebarGroupLabel>Collections</SidebarGroupLabel>
      <SidebarMenu>
        {isLoading && collections.length === 0
          ? [1, 2, 3, 4, 5].map((n) => (
              <SidebarMenuSkeleton
                key={`skeleton-${n}`}
                style={{ opacity: 1 - (n - 1) * 0.2 }}
              />
            ))
          : collections.map((item) => (
              <SidebarMenuItem key={item.name}>
                <SidebarMenuButton
                  isActive={
                    pathname === item.url || pathname.startsWith(item.url + "/")
                  }
                  render={<Link to={item.url} className="my-0.5" />}
                >
                  {item.icon}
                  <span>{item.name}</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            ))}
      </SidebarMenu>
    </SidebarGroup>
  );
}
