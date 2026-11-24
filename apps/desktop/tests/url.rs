//! Which page the desktop window opens (spec 010, "Clients"; spec 120: one app, at `/`).

use genesis_desktop::window_url;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn with_nothing_said_it_opens_the_app_on_the_local_serve() {
    assert_eq!(
        window_url(args(&[]), None).unwrap().as_str(),
        "http://127.0.0.1:8080/"
    );
}

#[test]
fn serve_can_be_anywhere() {
    let url = window_url(args(&["--serve", "https://genesis.example.com"]), None).unwrap();
    assert_eq!(url.as_str(), "https://genesis.example.com/");
}

#[test]
fn genesis_serve_in_the_environment_says_where_serve_is() {
    let url = window_url(args(&[]), Some("http://10.0.0.5:9000/".into())).unwrap();
    assert_eq!(url.as_str(), "http://10.0.0.5:9000/");
}

#[test]
fn the_window_opens_serves_root_even_when_the_address_has_a_path() {
    let url = window_url(args(&["--serve", "http://10.0.0.5:9000/old/page"]), None).unwrap();
    assert_eq!(url.as_str(), "http://10.0.0.5:9000/");
}

#[test]
fn an_app_name_is_refused_since_there_is_one_app() {
    let why = window_url(args(&["hello"]), None).unwrap_err();
    assert!(why.contains("hello") && why.contains("one app"), "{why}");
}

#[test]
fn a_serve_address_that_isnt_http_is_refused() {
    assert!(window_url(args(&["--serve", "file:///etc/passwd"]), None).is_err());
    assert!(window_url(args(&["--serve", "not a url"]), None).is_err());
}

#[test]
fn an_option_it_doesnt_know_is_refused() {
    assert!(
        window_url(args(&["--bogus"]), None)
            .unwrap_err()
            .contains("--bogus")
    );
}
