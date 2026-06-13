import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
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
  Field,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { getSite, updateSite } from "@/lib/api";

const siteSettingsSchema = z.object({
  name: z.string().min(1, "Site name is required"),
});

export function GeneralSection({
  siteId,
  canManage,
}: {
  siteId: string;
  canManage: boolean;
}) {
  const queryClient = useQueryClient();
  const [initialized, setInitialized] = useState(false);

  const { data: site, isLoading } = useQuery({
    queryKey: ["site", siteId],
    queryFn: () => getSite(siteId),
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

  return (
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
          <CardDescription>Basic information about this site.</CardDescription>
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
  );
}
