// The VIctory S mark: a geometric « S » (two bowls of radius 9 joined on a smooth spine, stroke 8, square
// ends) in near-black on a sunset-pink rounded square (corner radius 22 %). Drawn on a 64 x 64 grid; the
// app icon in src-tauri/build.rs rasterises the same geometry.

/** The S: top bowl centred (32, 23), bottom bowl centred (32, 41), both radius 9. */
export const LOGO_S_PATH = "M39.79 18.5 A9 9 0 1 0 32 32 A9 9 0 1 1 24.21 45.5"

export function Logo({ size = 36, className }: { size?: number; className?: string }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 64 64"
      className={className}
      role="img"
      aria-label="VIctory S"
      shapeRendering="geometricPrecision"
    >
      <rect width="64" height="64" rx="14" fill="#FF3C78" />
      <path d={LOGO_S_PATH} fill="none" stroke="#0B0B10" strokeWidth="8" strokeLinecap="butt" />
    </svg>
  )
}
