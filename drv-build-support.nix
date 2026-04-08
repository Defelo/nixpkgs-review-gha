{ configPath, drvGraphPath }:

let
  importJSON = path: builtins.fromJSON (builtins.readFile path);
  pipe = builtins.foldl' (acc: f: f acc);
  flip =
    f: x: y:
    f y x;
  unique = flip pipe [
    (map (x: {
      name = x;
      value = null;
    }))
    builtins.listToAttrs
    builtins.attrNames
  ];

  config = builtins.mapAttrs (_: { value, ... }: value) (importJSON configPath);
  graph = (importJSON drvGraphPath).derivations;

  systems = [ config.system ] ++ config.extra-platforms;
  features = config.system-features;

  toList =
    x:
    if builtins.isList x then
      x
    else if builtins.isString x then
      pipe x [
        (builtins.split "([^[:space:]]+)")
        (builtins.filter builtins.isList)
        (map builtins.head)
      ]
    else
      [ ];

  requiredFeatures =
    drv:
    pipe
      [
        drv.env or { }
        drv.structuredAttrs or { }
      ]
      [
        (builtins.concatMap (x: toList x.requiredSystemFeatures or [ ]))
        unique
      ];

  result = builtins.mapAttrs (_: drv: rec {
    inherit (drv) system;
    requiredSystemFeatures = requiredFeatures drv;
    systemSupported = system == "builtin" || builtins.elem system systems;
    featuresSupported = builtins.all (flip builtins.elem features) requiredSystemFeatures;
    dependenciesSupported = builtins.all (drvPath: result.${drvPath}.supported) (
      builtins.attrNames drv.inputs.drvs
    );
    supported = systemSupported && featuresSupported && dependenciesSupported;
  }) graph;
in

result
