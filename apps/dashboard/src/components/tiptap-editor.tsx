import Placeholder from "@tiptap/extension-placeholder";
import { EditorContent, useEditor, useEditorState } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import {
  Bold,
  Code,
  Heading1,
  Heading2,
  Heading3,
  ImageIcon,
  Italic,
  Link as LinkIcon,
  List,
  ListOrdered,
  Maximize2,
  Minimize2,
  Minus,
  Quote,
  Redo2,
  Strikethrough,
  Undo2,
} from "lucide-react";
import { useEffect, useState } from "react";
import { FilePickerDialog } from "@/components/file-picker-dialog";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { ImageWithControls } from "@/extensions/image-with-controls";
import "@/styles/tiptap.css";

interface TiptapEditorProps {
  content: string;
  onChange: (html: string) => void;
  placeholder?: string;
  siteId?: string;
  editable?: boolean;
}

export function TiptapEditor({
  content,
  onChange,
  placeholder = "Start writing...",
  siteId,
  editable = true,
}: TiptapEditorProps) {
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [filePickerOpen, setFilePickerOpen] = useState(false);

  useEffect(() => {
    if (isFullscreen) {
      document.body.style.overflow = "hidden";
      return () => {
        document.body.style.overflow = "";
      };
    }
  }, [isFullscreen]);

  const editor = useEditor({
    editable,
    extensions: [
      StarterKit.configure({
        link: {
          openOnClick: false,
          HTMLAttributes: { class: "text-primary underline" },
        },
      }),
      Placeholder.configure({ placeholder }),
      ImageWithControls.configure({ siteId }),
    ],
    content,
    editorProps: {
      attributes: {
        class: "prose prose-sm max-w-none focus:outline-none",
      },
    },
    onUpdate: ({ editor }) => {
      onChange(editor.getHTML());
    },
  });

  const editorState = useEditorState({
    editor,
    selector: (snapshot) => ({
      isBold: snapshot.editor.isActive("bold"),
      isItalic: snapshot.editor.isActive("italic"),
      isStrike: snapshot.editor.isActive("strike"),
      isH1: snapshot.editor.isActive("heading", { level: 1 }),
      isH2: snapshot.editor.isActive("heading", { level: 2 }),
      isH3: snapshot.editor.isActive("heading", { level: 3 }),
      isBulletList: snapshot.editor.isActive("bulletList"),
      isOrderedList: snapshot.editor.isActive("orderedList"),
      isBlockquote: snapshot.editor.isActive("blockquote"),
      isCodeBlock: snapshot.editor.isActive("codeBlock"),
      isLink: snapshot.editor.isActive("link"),
      canUndo: snapshot.editor.can().undo(),
      canRedo: snapshot.editor.can().redo(),
    }),
  });

  if (!editor) return null;

  const addLink = () => {
    const url = window.prompt("Enter URL:");
    if (url) {
      editor.chain().focus().setLink({ href: url }).run();
    }
  };

  const addImage = () => {
    if (siteId) {
      setFilePickerOpen(true);
    } else {
      const url = window.prompt("Enter image URL:");
      if (url) {
        editor.chain().focus().setImage({ src: url }).run();
      }
    }
  };

  const editorEl = (
    <div
      className={
        isFullscreen
          ? "fixed inset-0 z-50 flex flex-col bg-background"
          : "rounded-lg border"
      }
    >
      {editable && (
        <div className="flex flex-wrap items-center gap-0.5 border-b p-1">
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => editor.chain().focus().toggleBold().run()}
            aria-expanded={editorState.isBold}
            title="Bold"
          >
            <Bold />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => editor.chain().focus().toggleItalic().run()}
            aria-expanded={editorState.isItalic}
            title="Italic"
          >
            <Italic />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => editor.chain().focus().toggleStrike().run()}
            aria-expanded={editorState.isStrike}
            title="Strikethrough"
          >
            <Strikethrough />
          </Button>

          <Separator orientation="vertical" className="mx-1 h-6" />

          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() =>
              editor.chain().focus().toggleHeading({ level: 1 }).run()
            }
            aria-expanded={editorState.isH1}
            title="Heading 1"
          >
            <Heading1 />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() =>
              editor.chain().focus().toggleHeading({ level: 2 }).run()
            }
            aria-expanded={editorState.isH2}
            title="Heading 2"
          >
            <Heading2 />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() =>
              editor.chain().focus().toggleHeading({ level: 3 }).run()
            }
            aria-expanded={editorState.isH3}
            title="Heading 3"
          >
            <Heading3 />
          </Button>

          <Separator orientation="vertical" className="mx-1 h-6" />

          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => editor.chain().focus().toggleBulletList().run()}
            aria-expanded={editorState.isBulletList}
            title="Bullet List"
          >
            <List />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => editor.chain().focus().toggleOrderedList().run()}
            aria-expanded={editorState.isOrderedList}
            title="Ordered List"
          >
            <ListOrdered />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => editor.chain().focus().toggleBlockquote().run()}
            aria-expanded={editorState.isBlockquote}
            title="Blockquote"
          >
            <Quote />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => editor.chain().focus().toggleCodeBlock().run()}
            aria-expanded={editorState.isCodeBlock}
            title="Code Block"
          >
            <Code />
          </Button>

          <Separator orientation="vertical" className="mx-1 h-6" />

          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => editor.chain().focus().setHorizontalRule().run()}
            title="Horizontal Rule"
          >
            <Minus />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={addLink}
            aria-expanded={editorState.isLink}
            title="Link"
          >
            <LinkIcon />
          </Button>
          <Button variant="ghost" size="icon-sm" onClick={addImage} title="Image">
            <ImageIcon />
          </Button>

          <Separator orientation="vertical" className="mx-1 h-6" />

          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => editor.chain().focus().undo().run()}
            disabled={!editorState.canUndo}
            title="Undo"
          >
            <Undo2 />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => editor.chain().focus().redo().run()}
            disabled={!editorState.canRedo}
            title="Redo"
          >
            <Redo2 />
          </Button>

          <Button
            variant="ghost"
            size="icon-sm"
            className="ml-auto"
            onClick={() => setIsFullscreen((prev) => !prev)}
            title={isFullscreen ? "Exit fullscreen" : "Fullscreen"}
          >
            {isFullscreen ? <Minimize2 /> : <Maximize2 />}
          </Button>
        </div>
      )}
      <EditorContent
        editor={editor}
        className={isFullscreen ? "flex-1 overflow-y-auto p-4" : "p-4"}
      />

      {siteId && (
        <FilePickerDialog
          open={filePickerOpen}
          onOpenChange={setFilePickerOpen}
          onSelect={(file) => {
            editor.chain().focus().setImage({ src: file.url }).run();
          }}
          siteId={siteId}
          accept="image/*"
        />
      )}
    </div>
  );

  return editorEl;
}
