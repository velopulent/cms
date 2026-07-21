import { createFileRoute, Link } from "@tanstack/react-router";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
export const Route = createFileRoute("/_admin/sites/$siteId/settings/mcp")({
  component: McpSettings,
});
function McpSettings() {
  const { siteId } = Route.useParams();
  return (
    <div className="flex flex-col gap-4">
      <Card>
        <CardHeader>
          <CardTitle>MCP connection</CardTitle>
          <CardDescription>
            Connect an AI client using a personal access token. Your role and
            selected scopes limit every operation.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div>
            <p className="text-sm text-muted-foreground">Endpoint</p>
            <code className="block rounded-md bg-muted p-3 text-sm">{`${window.location.origin}/mcp`}</code>
          </div>
          <div>
            <p className="text-sm text-muted-foreground">Site ID</p>
            <code className="block rounded-md bg-muted p-3 text-sm">
              {siteId}
            </code>
          </div>
          <p className="text-sm text-muted-foreground">
            Create a PAT with <code>mcp.use</code> plus only needed content/file
            scopes. Use this site ID as explicit site context.
          </p>
          <Button className="w-fit" render={<Link to="/account" />}>
            Manage personal tokens
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
