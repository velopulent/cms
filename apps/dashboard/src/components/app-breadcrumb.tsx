import { Fragment } from "react";
import { useMatches, useParams, Link } from "@tanstack/react-router";
import { useQuery } from "@tanstack/react-query";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Skeleton } from "@/components/ui/skeleton";
import { getCollections, getEntryById, getSite, getSites } from "@/lib/api";

type BreadcrumbDef =
  | { label: string }
  | { labelFrom: "site" }
  | { labelFrom: "collection" }
  | { labelFrom: "singleton" }
  | { labelFrom: "entrySlug" };

interface BreadcrumbConfig {
  routeId: string;
  crumbs: BreadcrumbDef[];
}

const breadcrumbConfigs: BreadcrumbConfig[] = [
  {
    routeId: "/_admin/sites/$siteId/entries/$collectionSlug/$id/edit",
    crumbs: [{ labelFrom: "site" }, { labelFrom: "collection" }, { labelFrom: "entrySlug" }],
  },
  {
    routeId: "/_admin/sites/$siteId/entries/$collectionSlug/new",
    crumbs: [{ labelFrom: "site" }, { labelFrom: "collection" }, { label: "New" }],
  },
  {
    routeId: "/_admin/sites/$siteId/entries/$collectionSlug/",
    crumbs: [{ labelFrom: "site" }, { labelFrom: "collection" }],
  },
  {
    routeId: "/_admin/sites/$siteId/singletons/$slug",
    crumbs: [{ labelFrom: "site" }, { labelFrom: "singleton" }],
  },
  {
    routeId: "/_admin/sites/$siteId/collections",
    crumbs: [{ labelFrom: "site" }, { label: "Content Types" }],
  },
  {
    routeId: "/_admin/sites/$siteId/files",
    crumbs: [{ labelFrom: "site" }, { label: "Files" }],
  },
  {
    routeId: "/_admin/sites/$siteId/settings",
    crumbs: [{ labelFrom: "site" }, { label: "Settings" }],
  },
  {
    routeId: "/_admin/sites/$siteId/",
    crumbs: [{ labelFrom: "site" }],
  },
];

function useSiteName(siteId: string | undefined) {
  const { data: sites } = useQuery({
    queryKey: ["sites"],
    queryFn: getSites,
    enabled: !!siteId,
  });

  const siteFromList = sites?.find((s) => s.id === siteId);

  const { data: site } = useQuery({
    queryKey: ["site", siteId],
    queryFn: () => getSite(siteId!),
    enabled: !!siteId && !siteFromList,
  });

  return siteFromList?.name ?? site?.name;
}

function useCollectionName(siteId: string | undefined, collectionSlug: string | undefined) {
  const { data: collections } = useQuery({
    queryKey: ["collections", siteId],
    queryFn: () => getCollections(siteId!),
    enabled: !!siteId && !!collectionSlug,
  });

  return collections?.find((c) => c.slug === collectionSlug)?.name ?? collectionSlug;
}

function useSingletonName(siteId: string | undefined, slug: string | undefined) {
  const { data: collections } = useQuery({
    queryKey: ["collections", siteId],
    queryFn: () => getCollections(siteId!),
    enabled: !!siteId && !!slug,
  });

  return collections?.find((c) => c.is_singleton && c.slug === slug)?.name ?? slug;
}

function useEntrySlugLabel(siteId: string | undefined, entryId: string | undefined) {
  const { data: entry } = useQuery({
    queryKey: ["entry", siteId, entryId],
    queryFn: () => getEntryById(siteId!, entryId!),
    enabled: !!siteId && !!entryId,
  });

  return entry?.slug ?? entryId?.slice(0, 8);
}

function useBreadcrumbLabels(defs: BreadcrumbDef[], params: Record<string, string>) {
  const siteId = params.siteId;
  const siteName = useSiteName(siteId);
  const collectionName = useCollectionName(siteId, params.collectionSlug);
  const singletonName = useSingletonName(siteId, params.slug);
  const entrySlug = useEntrySlugLabel(siteId, params.id);

  return defs.map((def) => {
    if ("label" in def) return def.label;
    switch (def.labelFrom) {
      case "site":
        return siteName ?? null;
      case "collection":
        return collectionName ?? null;
      case "singleton":
        return singletonName ?? null;
      case "entrySlug":
        return entrySlug ?? null;
    }
  });
}

function buildHref(routeId: string, params: Record<string, string>, crumbIndex: number): string | undefined {
  const siteId = params.siteId;
  if (!siteId) return undefined;

  if (crumbIndex === 0) {
    return `/sites/${siteId}`;
  }

  if (routeId.includes("entries/$collectionSlug") && crumbIndex === 1) {
    return `/sites/${siteId}/entries/${params.collectionSlug}`;
  }

  return undefined;
}

export function AppBreadcrumb() {
  const matches = useMatches();
  const params = useParams({ strict: false }) as Record<string, string>;

  const currentMatch = matches[matches.length - 1];
  if (!currentMatch) return null;

  const routeId = currentMatch.routeId;
  const config = breadcrumbConfigs.find((c) => c.routeId === routeId);

  if (!config) {
    return (
      <Breadcrumb>
        <BreadcrumbList>
          <BreadcrumbItem>
            <BreadcrumbPage>—</BreadcrumbPage>
          </BreadcrumbItem>
        </BreadcrumbList>
      </Breadcrumb>
    );
  }

  const labels = useBreadcrumbLabels(config.crumbs, params);
  const isLoading = labels.some((label) => label === null);

  if (isLoading) {
    return (
      <Breadcrumb>
        <BreadcrumbList>
          {config.crumbs.map((_, i) => (
            <Fragment key={i}>
              {i > 0 && <BreadcrumbSeparator />}
              <BreadcrumbItem>
                <Skeleton className="h-4 w-20" />
              </BreadcrumbItem>
            </Fragment>
          ))}
        </BreadcrumbList>
      </Breadcrumb>
    );
  }

  return (
    <Breadcrumb>
      <BreadcrumbList>
        {labels.map((label, i) => {
          const isLast = i === labels.length - 1;
          const href = buildHref(routeId, params, i);

          return (
            <Fragment key={i}>
              {i > 0 && <BreadcrumbSeparator />}
              <BreadcrumbItem>
                {isLast ? (
                  <BreadcrumbPage>{label}</BreadcrumbPage>
                ) : href ? (
                  <BreadcrumbLink render={<Link to={href} />}>{label}</BreadcrumbLink>
                ) : (
                  <BreadcrumbPage>{label}</BreadcrumbPage>
                )}
              </BreadcrumbItem>
            </Fragment>
          );
        })}
      </BreadcrumbList>
    </Breadcrumb>
  );
}