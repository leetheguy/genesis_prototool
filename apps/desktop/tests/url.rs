//! Which page the desktop window opens (spec 010, "Clients").

use genesis_desktop::window_url;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn with_nothing_said_it_opens_the_app_list_on_the_local_serve() {
    assert_eq!(
        window_url(args(&[]), None).unwrap().as_str(),
        "http://127.0.0.1:8080/"
    );
}

#[test]
fn an_app_opens_that_apps_page() {
    assert_eq!(
        window_url(args(&["hello"]), None).unwrap().as_str(),
        "http://127.0.0.1:8080/hello/"
    );
}

#[test]
fn serve_can_be_anywhere() {
    let url = window_url(
        args(&["--serve", "https://genesis.example.com", "crm"]),
        None,
    )
    .unwrap();
    assert_eq!(url.as_str(), "https://genesis.example.com/crm/");
}

#[test]
fn genesis_serve_in_the_environment_says_where_serve_is() {
    let url = window_url(args(&["crm"]), Some("http://10.0.0.5:9000/".into())).unwrap();
    assert_eq!(url.as_str(), "http://10.0.0.5:9000/crm/");
}

#[test]
fn an_app_name_that_isnt_plain_is_refused() {
    assert!(
        window_url(args(&["../etc"]), None)
            .unwrap_err()
            .contains("../etc")
    );
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

#[test]
fn app_names_may_hold_underscores_and_dashes() {
    assert_eq!(
        window_url(args(&["my_app-2"]), None).unwrap().as_str(),
        "http://127.0.0.1:8080/my_app-2/"
    );
}
