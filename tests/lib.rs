use std::thread;
use std::time::{Instant, Duration};
extern crate coroutine;
extern crate generator;

use coroutine::yield_now;
use generator::Gn;

#[test]
fn panic_coroutine() {
    let j = coroutine::spawn(move || {
        panic!("panic inside coroutine");
    });
    match j.join() {
        Ok(_) => panic!("test should return panic"),
        Err(panic) => {
            match panic.downcast_ref::<&str>() {
                Some(e) => return println!("Panicked inside: {:?}", e),
                None => panic!("panic type wrong"),
            }
        }
    }
}

#[test]
fn cancel_coroutine() {
    let j = coroutine::spawn(move || {
        // suspend the coroutine to simulate an endless loop
        println!("before cancel");
        coroutine::park();
        println!("canceled, should not come here");
        // next 'sys call would cause a Cancel panic'
        coroutine::sleep(Duration::from_secs(1000000));
    });

    // let the coroutine run
    thread::sleep(Duration::from_millis(10));

    unsafe { j.coroutine().cancel() };

    match j.join() {
        Ok(_) => panic!("test should return panic"),
        Err(panic) => {
            match panic.downcast_ref::<&str>() {
                Some(e) => return println!("Panicked inside: {:?}", e),
                None => panic!("panic type wrong"),
            }
        }
    }
}

#[test]
fn cancel_io_coroutine() {
    let j = coroutine::spawn(move || {
        let listener = coroutine::net::TcpListener::bind(("0.0.0.0", 1234)).unwrap();
        println!("listening on {:?}", listener.local_addr().unwrap());

        for stream in listener.incoming() {
            match stream {
                Ok(_s) => println!("got a connection"),
                Err(e) => println!("err = {:?}", e),
            }
        }
    });

    // let the coroutine run
    thread::sleep(Duration::from_millis(10));

    unsafe { j.coroutine().cancel() };

    match j.join() {
        Ok(_) => panic!("test should return panic"),
        Err(panic) => {
            match panic.downcast_ref::<&str>() {
                Some(e) => return println!("Panicked inside: {:?}", e),
                None => panic!("panic type wrong"),
            }
        }
    }
}

#[test]
fn one_coroutine() {
    let j = coroutine::spawn(move || {
        println!("hello, coroutine");
    });
    j.join().unwrap();
}

#[test]
fn coroutine_result() {
    let j = coroutine::spawn(move || {
        println!("hello, coroutine");
        100
    });

    assert_eq!(j.join().unwrap(), 100);
}

#[test]
fn multi_coroutine() {
    for i in 0..10 {
        coroutine::spawn(move || {
            println!("hi, coroutine{}", i);
        });
    }
    thread::sleep(Duration::from_millis(200));
}

#[test]
fn test_yield() {
    let j = coroutine::spawn(move || {
        println!("hello, coroutine");
        yield_now();
        println!("goodbye, coroutine");
    });
    j.join().unwrap();
}

#[test]
fn multi_yield() {
    for i in 0..10 {
        coroutine::spawn(move || {
            println!("hi, coroutine{}", i);
            yield_now();
            println!("bye, coroutine{}", i);
        });
    }
    thread::sleep(Duration::from_millis(200));
}

#[test]
fn spawn_inside() {
    coroutine::Builder::new()
        .name("parent".to_owned())
        .spawn(move || {
            let me = coroutine::current();
            println!("hi, I'm parent: {:?}", me);
            for i in 0..10 {
                coroutine::spawn(move || {
                    println!("hi, I'm child{:?}", i);
                    yield_now();
                    println!("bye from child{:?}", i);
                });
            }
            yield_now();
            println!("bye from parent: {:?}", me);
        })
        .unwrap()
        .join()
        .unwrap();
    thread::sleep(Duration::from_millis(200));
}

#[test]
fn wait_join() {
    let j = coroutine::spawn(move || {
        println!("hi, I'm parent");
        let join = (0..10)
            .map(|i| {
                coroutine::spawn(move || {
                    println!("hi, I'm child{:?}", i);
                    yield_now();
                    println!("bye from child{:?}", i);
                })
            })
            .collect::<Vec<_>>();

        for j in join {
            j.join().unwrap();
        }
        println!("bye from parent");
    });
    j.join().unwrap();
}

#[test]
fn scoped_coroutine() {
    let mut array = [1, 2, 3];
    coroutine::scope(|scope| {
        for i in &mut array {
            scope.spawn(move || {
                *i += 1;
            });
        }
    });

    assert_eq!(array[0], 2);
    assert_eq!(array[1], 3);
    assert_eq!(array[2], 4);
}

#[test]
fn yield_from_gen() {
    let mut a = 0;
    coroutine::scope(|scope| {
        scope.spawn(|| {
            let g = Gn::<()>::new_scoped(|mut scope| {
                while a < 10 {
                    scope.yield_(a);
                    // this is yield from the generator context!
                    yield_now();
                    a += 1;
                }
                a
            });
            g.fold((), |_, i| {
                println!("got {:?}", i);
            });
        });
    });

    assert_eq!(a, 10);
}

#[test]
fn unpark() {
    let mut a = 0;
    coroutine::scope(|scope| {
        let h = scope.spawn(|| {
            let co = coroutine::current();
            println!("child coroutine name:{:?}", co);
            co.unpark();
            a = 5;
            coroutine::park();
            a = 10;
            coroutine::park();
        });

        // seems this could happend before the inner unpark
        // it depends on how many coroutines are running
        // if the test_sleep spawns too many within the same time span
        // this could be fail due to schedule latency. default 1 worker
        thread::sleep(Duration::from_millis(10));
        h.coroutine().unpark();
    });

    assert_eq!(a, 10);
}

#[test]
fn park_timeout() {
    let mut a = 0;
    coroutine::scope(|scope| {
        let h = scope.spawn(|| {
            let co = coroutine::current();
            co.unpark();
            a = 5;
            coroutine::park_timeout(Duration::from_millis(100));
            let now = Instant::now();
            coroutine::park_timeout(Duration::from_millis(10));
            assert!(now.elapsed() >= Duration::from_millis(10));
            let now = Instant::now();
            coroutine::park_timeout(Duration::from_millis(100));
            assert!(now.elapsed() < Duration::from_millis(100));
            a = 10;
        });

        thread::sleep(Duration::from_millis(50));
        h.coroutine().unpark();
    });

    assert_eq!(a, 10);
}

#[test]
fn test_sleep() {
    let now = Instant::now();
    coroutine::sleep(Duration::from_millis(500));
    assert!(now.elapsed() >= Duration::from_millis(500));

    coroutine::scope(|scope| {
        for _ in 0..1000 {
            scope.spawn(|| {
                let now = Instant::now();
                coroutine::sleep(Duration::from_millis(100));
                assert!(now.elapsed() >= Duration::from_millis(100));
            });
        }
    });
}
