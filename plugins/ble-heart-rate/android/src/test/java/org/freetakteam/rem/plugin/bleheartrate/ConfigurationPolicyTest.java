package org.freetakteam.rem.plugin.bleheartrate;

import static org.junit.Assert.*;
import org.junit.Test;

public final class ConfigurationPolicyTest {
    @Test
    public void trimsNonEmptyAliases() {
        assertEquals("Medic 1", ConfigurationPolicy.requireAlias("  Medic 1  "));
    }

    @Test(expected = IllegalArgumentException.class)
    public void rejectsEmptyAliases() {
        ConfigurationPolicy.requireAlias("   ");
    }
}
