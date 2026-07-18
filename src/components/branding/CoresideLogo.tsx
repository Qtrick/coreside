import { inAppLogoFor, type BrandAppearance } from "@/assets/branding/manifest";

type CoresideLogoProps = {
  appearance: BrandAppearance;
  size?: number;
  className?: string;
  alt?: string;
};

/** Protected in-app Coreside mark — follows resolved Appearance theme. */
export function CoresideLogo({
  appearance,
  size = 28,
  className,
  alt = "Coreside",
}: CoresideLogoProps) {
  return (
    <img
      src={inAppLogoFor(appearance)}
      alt={alt}
      width={size}
      height={size}
      className={className ? `coreside-logo ${className}` : "coreside-logo"}
      draggable={false}
    />
  );
}
