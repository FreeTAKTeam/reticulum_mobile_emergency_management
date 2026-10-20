package org.freetakteam.rem.plugin.watchstatus;

import org.json.JSONException;
import org.json.JSONObject;

final class RadarFailure extends Exception {
    final String code;
    final int httpStatus;

    RadarFailure(String code, int httpStatus) {
        super(code);
        this.code = code;
        this.httpStatus = httpStatus;
    }

    String json(String requestId) {
        try {
            return new JSONObject().put("protocolVersion", 1).put("requestId", requestId)
                .put("ok", false).put("error", new JSONObject().put("code", code)).toString();
        } catch (JSONException impossible) {
            return "{\"protocolVersion\":1,\"ok\":false,\"error\":{\"code\":\"HostError\"}}";
        }
    }
}
