// The Sagouindupm mascot: a sagouin (common marmoset) on the sunset-pink rounded square (corner radius 22 %).
// The drawings are src/assets/sagouin.svg (32 px and up) and src/assets/sagouin-small.svg (below: face
// silhouette, ear tufts, blaze and eyes only, what still reads at 16 px). The favicon (index.html) and the app
// icon (src-tauri/build.rs renders the .ico from them) use the same two files.
import detailed from "@/assets/sagouin.svg?raw"
import small from "@/assets/sagouin-small.svg?raw"

/** What is inside a file's <svg> element (its drawing, without the outer element). */
function inner(svg: string): string {
  return svg.slice(svg.indexOf(">", svg.indexOf("<svg")) + 1, svg.lastIndexOf("</svg>"))
}

const DETAILED = inner(detailed)
const SMALL = inner(small)

export function Logo({ size = 36, className }: { size?: number; className?: string }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 64 64"
      className={className}
      role="img"
      aria-label="Sagouindupm"
      shapeRendering="geometricPrecision"
      dangerouslySetInnerHTML={{ __html: size >= 32 ? DETAILED : SMALL }}
    />
  )
}
