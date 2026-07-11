package org.freetakteam.rem.plugin.bleheartrate;

final class ConfigurationPolicy {
    private ConfigurationPolicy() {}

    static String requireAlias(String value) {
        final String alias = value == null ? "" : value.trim();
        if (alias.isEmpty()) {
            throw new IllegalArgumentException("Alias is required");
        }
        return alias;
    }
}
