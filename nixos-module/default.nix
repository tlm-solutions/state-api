{ pkgs, config, lib, ... }:
let
  cfg = config.TLMS.api;
in
{
  options.TLMS.api = with lib; {
    enable = mkOption {
      type = types.bool;
      default = false;
      description = "enabling the service";
    };
    GRPC.host = mkOption {
      type = types.str;
      default = "127.0.0.1";
      description = "grpc host";
    };
    GRPC.port = mkOption {
      type = types.int;
      default = 50051;
      description = "grpc port";
    };
    port = mkOption {
      type = types.port;
      default = 9001;
      description = "port of the api";
    };
    graphFile = mkOption {
      type = types.either types.path types.str;
      default = "";
      description = "location of the graph file";
    };
    stopsFile = mkOption {
      type = types.either types.path types.str;
      default = "";
      description = "location of the stops file";
    };
    user = mkOption {
      type = types.str;
      default = "dvb-api";
      description = "as which user dvb-api should run";
    };
    group = mkOption {
      type = types.str;
      default = "dvb-api";
      description = "as which group dvb-api should run";
    };
    logLevel = mkOption {
      type = types.str;
      default = "info";
      description = "log level";
    };
    workerCount = mkOption {
      type = types.int;
      default = 4;
      description = "amount of worker threads used";
    };
  };
  config = lib.mkIf cfg.enable {
    systemd = {
      services = {
        "dvb-api" = {
          enable = true;
          wantedBy = [ "multi-user.target" ];

          script = "exec ${pkgs.state-api}/bin/state-api &";

          environment = {
            "RUST_LOG" = "${cfg.logLevel}";
            "GRPC_HOST" = "${cfg.GRPC.host}:${toString cfg.GRPC.port}";
            "HTTP_PORT" = "${toString cfg.port}";
            "GRAPH_FILE" = "${cfg.graphFile}";
            "STOPS_FILE" = "${cfg.stopsFile}";
            "WORKER_COUNT" = "${toString cfg.workerCount}";
          };

          serviceConfig = {
            Type = "forking";
            User = "${cfg.user}";
            Restart = "always";
          };
        };
      };
    };

    # user accounts for systemd units
    users.users."${cfg.user}" = {
      name = "${cfg.user}";
      description = "public dvb api service";
      group = "${cfg.group}";
      isSystemUser = true;
      extraGroups = [ ];
    };
  };
}
