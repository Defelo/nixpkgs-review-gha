final: prev: {
  nixpkgs-review = prev.nixpkgs-review.overrideAttrs (attrs: {
    patches = attrs.patches or [ ] ++ [
      ./get_pr_from_environ.patch
      ./use_api_revs.patch
    ];
  });
}
