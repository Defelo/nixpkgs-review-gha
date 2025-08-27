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
  graph = importJSON drvGraphPath;

  systems = [ config.system ] ++ config.extra-platforms;
  features = config.system-features;

  toList =
    x:
    if builtins.isList x then
      x
    else if builtins.isString x then
      builtins.filter builtins.isString (builtins.split " " x)
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
      builtins.attrNames drv.inputDrvs
    );
    supported = systemSupported && featuresSupported && dependenciesSupported;
  }) graph;
in

result
