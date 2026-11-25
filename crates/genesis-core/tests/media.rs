//! `audio`, `video`, and `iframe` (spec 040, "Media and embeds").

use genesis_core::bus::Machine;
use genesis_core::config::{App, ConfigError, Node};

fn app(node: &str) -> Result<App, Vec<ConfigError>> {
    App::from_toml(&format!("[app]\nname = \"x\"\n\n[[body]]\n{node}\n"))
}

fn problem(node: &str) -> String {
    app(node)
        .unwrap_err()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Loads `node`, checks it survives the trip to the browser (JSON), and hands it back.
fn round_trip(node: &str) -> Node {
    let app = app(node).unwrap_or_else(|e| panic!("{node}: {e:?}"));
    let back: App = serde_json::from_value(serde_json::to_value(&app).unwrap()).unwrap();
    assert_eq!(back, app, "{node}");
    app.body[0].clone()
}

#[test]
fn audio_has_controls_and_nothing_else_unless_it_says_so() {
    let Node::Audio(audio) = round_trip("type = \"audio\"\nsrc = \"/fs/{{ app.track }}.mp3\"")
    else {
        panic!("not audio")
    };
    assert_eq!(
        (audio.controls, audio.autoplay, audio.r#loop, audio.muted),
        (true, false, false, false)
    );
    let Node::Audio(audio) = round_trip(
        "type = \"audio\"\nsrc = \"/a.mp3\"\ncontrols = false\nautoplay = true\nloop = true\nmuted = true\nspan = 3",
    ) else {
        panic!("not audio")
    };
    assert_eq!(
        (audio.controls, audio.autoplay, audio.r#loop, audio.muted),
        (false, true, true, true)
    );
}

#[test]
fn video_is_audio_with_a_poster_and_the_box_keys() {
    let Node::Video(video) = round_trip(
        "type = \"video\"\nsrc = \"/v.mp4\"\nposter = \"/v.png\"\nmuted = true\nradius = \"box\"\nwidth = \"full\"",
    ) else {
        panic!("not video")
    };
    assert_eq!(video.poster.as_deref(), Some("/v.png"));
    assert!(video.muted);
    assert!(
        problem("type = \"audio\"\nsrc = \"/a.mp3\"\nposter = \"/p.png\"")
            .contains("unknown field `poster`")
    );
}

#[test]
fn media_with_an_id_lists_play_pause_and_ended_among_what_the_app_fires() {
    let video = app("type = \"video\"\nid = \"intro\"\nsrc = \"/v.mp4\"").unwrap();
    let fires = Machine::new(video).get("sys.bus").unwrap()["fires"].clone();
    for name in ["intro.play", "intro.pause", "intro.ended"] {
        assert!(
            fires.as_array().unwrap().iter().any(|f| f == name),
            "{name}: {fires}"
        );
    }
    let audio = app("type = \"audio\"\nsrc = \"/a.mp3\"").unwrap();
    let fires = Machine::new(audio).get("sys.bus").unwrap()["fires"].to_string();
    assert!(!fires.contains(".play"), "no id, nothing fired: {fires}");
}

#[test]
fn an_iframe_needs_a_title_and_is_sandboxed_unless_it_says_otherwise() {
    assert!(
        problem("type = \"iframe\"\nsrc = \"https://example.com\"")
            .contains("missing field `title`")
    );
    let Node::Iframe(frame) = round_trip(
        "type = \"iframe\"\nsrc = \"https://example.com/{{ app.id }}\"\ntitle = \"Map\"",
    ) else {
        panic!("not an iframe")
    };
    assert!(frame.sandbox);
    let Node::Iframe(frame) = round_trip(
        "type = \"iframe\"\nsrc = \"/report\"\ntitle = \"Report\"\nsandbox = false\nborder = true\nspan = 6",
    ) else {
        panic!("not an iframe")
    };
    assert!(!frame.sandbox);
}

#[test]
fn an_iframes_height_is_the_box_keys_height() {
    for height in [
        "\"full\"",
        "\"screen\"",
        "4",
        "24",
        "64",
        "{ base = 16, md = 32 }",
    ] {
        round_trip(&format!(
            "type = \"iframe\"
src = \"/\"
title = \"t\"
height = {height}"
        ));
    }
    for height in ["\"tall\"", "0", "30", "2.5"] {
        assert_eq!(
            problem(&format!(
                "type = \"iframe\"
src = \"/\"
title = \"t\"
height = {height}"
            )),
            format!(
                "app.toml:8:1: `height` is \"full\", \"screen\", 4, 8, 12, 16, 24, 32, 48, or 64, not {height}"
            ),
        );
    }
}
