import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { z } from "zod";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Field,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { deleteSite, getSite, updateSite } from "@/lib/api";

const siteSettingsSchema = z.object({
  name: z.string().min(1, "Site name is required"),
});

export function GeneralSection({
  siteId,
  canManage,
  role,
}: {
  siteId: string;
  canManage: boolean;
  role: "editor" | "viewer";
}) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [initialized, setInitialized] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const { data: site, isLoading } = useQuery({
    queryKey: ["site", siteId],
    queryFn: () => getSite(siteId),
  });

  const deleteMutation = useMutation({
    mutationFn: () => deleteSite(siteId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["sites"] });
      setConfirmDelete(false);
      toast.success("Site deleted");
      navigate({ to: "/" });
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const form = useForm({
    defaultValues: { name: "" },
    validators: { onSubmit: siteSettingsSchema },
    onSubmit: async ({ value }) => {
      updateMutation.mutate(value);
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({ name }: { name: string }) => updateSite(siteId, { name }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["site", siteId] });
      queryClient.invalidateQueries({ queryKey: ["sites"] });
      toast.success("Site settings updated");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  useEffect(() => {
    if (site && !initialized) {
      form.reset();
      form.setFieldValue("name", site.name);
      setInitialized(true);
    }
  }, [site, initialized, form]);

  if (isLoading || !initialized) {
    return <Skeleton className="h-48 w-full max-w-2xl" />;
  }

  if (!site) {
    return <p className="text-sm text-muted-foreground">Site not found.</p>;
  }

  if (!canManage)
    return (
      <Card>
        <CardHeader>
          <CardTitle>General</CardTitle>
          <CardDescription>
            Site information available to your account.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div>
            <p className="text-sm text-muted-foreground">Site name</p>
            <p className="font-medium">{site.name}</p>
          </div>
          <div>
            <p className="text-sm text-muted-foreground">Site ID</p>
            <p className="font-mono text-sm">{site.id}</p>
          </div>
          <div>
            <p className="text-sm text-muted-foreground">Your role</p>
            <p className="capitalize font-medium">{role}</p>
          </div>
        </CardContent>
      </Card>
    );

  return (
    <div className="flex flex-col gap-6">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          form.handleSubmit();
        }}
        className="flex flex-col gap-6"
      >
        <Card>
          <CardHeader>
            <CardTitle>General</CardTitle>
            <CardDescription>
              Basic information about this site.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <FieldGroup>
              <form.Field
                name="name"
                children={(field) => {
                  const isInvalid =
                    field.state.meta.isTouched && !field.state.meta.isValid;
                  return (
                    <Field data-invalid={isInvalid}>
                      <FieldLabel htmlFor={field.name}>Site Name</FieldLabel>
                      <Input
                        id={field.name}
                        name={field.name}
                        placeholder="My Site"
                        value={field.state.value}
                        onBlur={field.handleBlur}
                        onChange={(e) => field.handleChange(e.target.value)}
                        className="max-w-md"
                        aria-invalid={isInvalid}
                        disabled={!canManage}
                      />
                      {isInvalid && (
                        <FieldError errors={field.state.meta.errors} />
                      )}
                    </Field>
                  );
                }}
              />
            </FieldGroup>
          </CardContent>
        </Card>

        <Button
          type="submit"
          className="w-fit"
          disabled={!canManage || updateMutation.isPending}
        >
          {!canManage
            ? "Admin access required"
            : updateMutation.isPending
              ? "Saving..."
              : "Save Changes"}
        </Button>
      </form>

      {canManage && (
        <Card className="border-destructive/40">
          <CardHeader>
            <CardTitle>Danger zone</CardTitle>
            <CardDescription>
              Deleting a site permanently removes its content, files, schema,
              and members. This cannot be undone.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Button
              type="button"
              variant="destructive"
              onClick={() => setConfirmDelete(true)}
            >
              Delete site
            </Button>
          </CardContent>
        </Card>
      )}

      <Dialog open={confirmDelete} onOpenChange={setConfirmDelete}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete site</DialogTitle>
            <DialogDescription>
              Permanently delete <strong>{site.name}</strong> and all of its
              content, files, and members. This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <DialogClose render={<Button variant="outline" />}>
              Cancel
            </DialogClose>
            <Button
              variant="destructive"
              disabled={deleteMutation.isPending}
              onClick={() => deleteMutation.mutate()}
            >
              {deleteMutation.isPending ? "Deleting..." : "Delete site"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
